//! Unit tests for `split_divider` composite (`UIF-COMP-016`, `UIF-PANE-001`, `UIF-PRIN-006`).

use iced::advanced::Shell;
use iced::advanced::clipboard::Null;
use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::widget::Tree;
use iced::advanced::widget::Widget;
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::mouse;
use iced::{Background, Event, Length, Point, Rectangle, Size};

use crate::binding::snapshot::GestureStart;
use crate::composite::split_divider::{
    SplitAxis, SplitDivider, SplitDividerModel, SplitDividerResize, SplitDividerState,
    compute_grab_rect, split_divider, split_divider_geometry, split_divider_layers,
};
use crate::state::{BaseState, VisualState};
use crate::test_helpers::{Harness, headless_renderer};
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
fn test_resting_layer_tokens_per_axis() {
    let palette = Palette::dark();
    let visual = VisualState {
        base: BaseState::Rest,
        focused: false,
    };

    // (case label, axis, divider box, resting stroke, grip pill, grab zone)
    let cases = [
        (
            "horizontal split",
            SplitAxis::Horizontal,
            Rectangle::new(Point::new(100.0, 50.0), Size::new(10.0, 500.0)),
            Rectangle::new(Point::new(104.5, 50.0), Size::new(1.0, 500.0)),
            Rectangle::new(
                Point::new(104.0, 50.0 + (500.0 - size::HIT_TARGET_MIN) / 2.0),
                Size::new(space::XXS, size::HIT_TARGET_MIN),
            ),
            Rectangle::new(
                Point::new(100.0, 50.0),
                Size::new(size::SPLITTER_GRAB, 500.0),
            ),
        ),
        (
            "vertical split",
            SplitAxis::Vertical,
            Rectangle::new(Point::new(50.0, 100.0), Size::new(600.0, 10.0)),
            Rectangle::new(Point::new(50.0, 104.5), Size::new(600.0, 1.0)),
            Rectangle::new(
                Point::new(50.0 + (600.0 - size::HIT_TARGET_MIN) / 2.0, 104.0),
                Size::new(size::HIT_TARGET_MIN, space::XXS),
            ),
            Rectangle::new(
                Point::new(50.0, 100.0),
                Size::new(600.0, size::SPLITTER_GRAB),
            ),
        ),
    ];

    for (label, axis, bounds, stroke, pill, grab) in cases {
        let layers = split_divider_layers(bounds, axis, visual, &palette);
        assert_eq!(
            layers.len(),
            2,
            "[{label}] a resting divider must paint a stroke and a grip pill"
        );
        assert_eq!(
            layers[0].0, "divider-stroke",
            "[{label}] the first layer must be the stroke"
        );
        assert_eq!(
            layers[1].0, "grip-pill",
            "[{label}] the second layer must be the grip pill"
        );

        // 1px stroke in color.border.subtle, centred on the divider box
        assert_eq!(
            layers[0].1.bounds, stroke,
            "[{label}] the resting stroke must be the 1px centred line"
        );
        assert_eq!(
            layers[0].1.fill, palette.border_subtle,
            "[{label}] the resting stroke must be color.border.subtle"
        );

        // Grip pill in color.border.strong, centred along the stroke, fully rounded
        assert_eq!(
            layers[1].1.bounds, pill,
            "[{label}] the grip pill must be centred on the split stroke"
        );
        assert_eq!(
            layers[1].1.fill, palette.border_strong,
            "[{label}] the resting grip pill must be color.border.strong"
        );
        assert_eq!(
            layers[1].1.border.radius,
            shape::RADIUS_PILL.into(),
            "[{label}] the grip pill must use the pill radius"
        );

        // Grab zone spans size.splitter.grab across the axis
        assert_eq!(
            compute_grab_rect(bounds, axis),
            grab,
            "[{label}] the grab zone must be a {grab_w}px band centred on the axis",
            grab_w = size::SPLITTER_GRAB
        );
    }
}

#[test]
fn test_hover_and_drag_promotion_accent_border() {
    let palette = Palette::dark();
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(10.0, 300.0));

    // (case label, resolved base state)
    let cases = [
        ("hovered", BaseState::Hovered),
        // An active drag resolves to `Pressed`.
        ("active drag", BaseState::Pressed),
    ];

    for (label, base) in cases {
        let visual = VisualState {
            base,
            focused: false,
        };
        let layers = split_divider_layers(bounds, SplitAxis::Horizontal, visual, &palette);

        assert_eq!(
            layers[0].1.fill, palette.accent_border,
            "[{label}] the promoted stroke must be color.accent.border"
        );
        assert_eq!(
            layers[1].1.fill, palette.accent_border,
            "[{label}] the promoted grip pill must be color.accent.border"
        );
    }

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

// ---------------------------------------------------------------------------
// Ratio clamping edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_clamp_ratio_repairs_an_inverted_band() {
    // A ceiling below the floor is a configuration error: the floor wins and
    // the clamp never panics.
    assert_eq!(
        SplitDividerModel::clamp_ratio(0.5, 0.8, 0.2),
        0.8,
        "an inverted band must collapse onto the floor rather than panic"
    );

    // Out-of-range bounds are pulled back into [0, 1] before the value is clamped.
    assert_eq!(
        SplitDividerModel::clamp_ratio(0.5, -0.5, 2.0),
        0.5,
        "bounds outside [0, 1] must not narrow the band"
    );
    assert_eq!(SplitDividerModel::clamp_ratio(5.0, 0.0, 1.0), 1.0);
    assert_eq!(SplitDividerModel::clamp_ratio(-5.0, 0.0, 1.0), 0.0);

    // The builders store the requested ceiling verbatim, and `clamped_ratio`
    // repairs the inversion the same way.
    let inverted = SplitDividerModel::new(SplitAxis::Horizontal, 0.5)
        .with_min_ratio(0.6)
        .with_max_ratio(0.2);
    assert_eq!(
        inverted.max_ratio, 0.2,
        "the builder keeps the ceiling it was given"
    );
    assert_eq!(
        inverted.clamped_ratio(),
        0.6,
        "an inverted model band must resolve to the floor rather than panic"
    );
}

#[test]
fn test_a_fresh_resize_starts_idle_and_clamped() {
    // (case label, requested ratio, expected captured ratio)
    let cases = [
        ("above the ceiling", 1.5, 0.8),
        ("below the floor", -0.5, 0.2),
        ("inside the band", 0.5, 0.5),
    ];

    for (label, requested, expected) in cases {
        let resize = SplitDividerResize::new(SplitAxis::Vertical, requested, 0.2, 0.8);

        assert_eq!(
            resize.axis(),
            SplitAxis::Vertical,
            "[{label}] the gesture must report the axis it was built for"
        );
        assert!(
            !resize.is_active(),
            "[{label}] a freshly built gesture must be idle"
        );
        assert_eq!(
            resize.start_ratio(),
            expected,
            "[{label}] the start ratio must be clamped into the band"
        );
        assert_eq!(
            resize.current_ratio(),
            expected,
            "[{label}] the live ratio must begin at the clamped ratio"
        );
    }
}

#[test]
fn test_begin_clamps_the_ratio_it_captures() {
    let mut resize = SplitDividerResize::new(SplitAxis::Horizontal, 0.5, 0.2, 0.8);

    assert_eq!(
        resize.begin(1.4, 100.0, 1000.0, 0.2, 0.8),
        GestureStart::Started,
        "an idle gesture must open"
    );
    assert_eq!(
        resize.start_ratio(),
        0.8,
        "the snapshot must capture the clamped ratio, not the requested one"
    );
    assert_eq!(resize.current_ratio(), 0.8);

    assert!(
        resize.update(100.0).is_none(),
        "a tick with zero travel must not emit a live value"
    );

    let cancel = resize.cancel().expect("an open gesture must cancel");
    assert_eq!(
        cancel.restored().ratio,
        0.8,
        "the snapshot must restore the clamped start ratio"
    );
}

#[test]
fn test_an_idle_gesture_emits_no_live_commit_or_cancel() {
    let mut resize = SplitDividerResize::new(SplitAxis::Horizontal, 0.5, 0.1, 0.9);

    assert!(
        resize.update(500.0).is_none(),
        "a move outside a gesture must be ignored"
    );
    assert!(
        resize.commit().is_none(),
        "a release outside a gesture must not commit"
    );
    assert!(
        resize.cancel().is_none(),
        "a cancel outside a gesture must not restore"
    );
    assert_eq!(
        resize.current_ratio(),
        0.5,
        "idle operations must not move the ratio"
    );
}

#[test]
fn test_abort_closes_the_gesture_without_a_callback() {
    let mut resize = SplitDividerResize::new(SplitAxis::Horizontal, 0.5, 0.0, 1.0);
    assert_eq!(
        resize.begin(0.5, 100.0, 1000.0, 0.0, 1.0),
        GestureStart::Started
    );
    assert!(
        resize.update(200.0).is_some(),
        "the first tick must move the ratio"
    );

    resize.abort();

    assert!(!resize.is_active(), "abort must close the gesture slot");
    assert!(
        !resize.gate().is_pending(),
        "abort must settle the activation gate"
    );
    assert!(
        resize.cancel().is_none(),
        "a settled gesture must not cancel a second time"
    );
    assert!(
        resize.commit().is_none(),
        "a settled gesture must not commit"
    );
    assert!(
        resize.update(300.0).is_none(),
        "a move after abort must be ignored until a new gesture opens"
    );

    assert_eq!(
        resize.begin(0.5, 100.0, 1000.0, 0.0, 1.0),
        GestureStart::Started,
        "abort must leave the activation gate reusable"
    );
}

// ---------------------------------------------------------------------------
// Geometry edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_the_grip_pill_shrinks_to_a_short_divider() {
    // A divider shorter than the minimum hit target must not paint a pill that
    // hangs off its own box.
    let short = size::HIT_TARGET_MIN / 3.0;

    // (case label, axis, divider box, expected pill size)
    let cases = [
        (
            "horizontal split",
            SplitAxis::Horizontal,
            Rectangle::new(
                Point::new(0.0, 100.0),
                Size::new(size::SPLITTER_GRAB, short),
            ),
            Size::new(space::XXS, short),
        ),
        (
            "vertical split",
            SplitAxis::Vertical,
            Rectangle::new(
                Point::new(100.0, 0.0),
                Size::new(short, size::SPLITTER_GRAB),
            ),
            Size::new(short, space::XXS),
        ),
    ];

    for (label, axis, bounds, expected_pill) in cases {
        let (stroke, pill) = split_divider_geometry(bounds, axis);

        assert_eq!(
            pill.size(),
            expected_pill,
            "[{label}] the pill must be capped at the divider's own length"
        );
        assert!(
            bounds.contains(pill.center()),
            "[{label}] the clamped pill must stay inside the divider box"
        );
        assert_eq!(
            stroke.width.min(stroke.height),
            1.0,
            "[{label}] the stroke must stay 1px thick"
        );
    }
}

#[test]
fn test_negative_bounds_produce_no_layers() {
    let palette = Palette::dark();
    let visual = VisualState {
        base: BaseState::Rest,
        focused: false,
    };

    // (case label, degenerate divider size)
    let sizes = [
        ("negative width", Size::new(-10.0, 300.0)),
        ("negative height", Size::new(10.0, -1.0)),
    ];

    for (label, box_size) in sizes {
        for axis in [SplitAxis::Horizontal, SplitAxis::Vertical] {
            let layers = split_divider_layers(
                Rectangle::new(Point::ORIGIN, box_size),
                axis,
                visual,
                &palette,
            );
            assert!(
                layers.is_empty(),
                "[{label}] a degenerate divider must emit no layers rather than an inverted quad"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Widget protocol: tag, state, size, layout, draw
// ---------------------------------------------------------------------------

#[test]
fn test_widget_tag_state_and_size_per_axis() {
    // (case label, axis, expected size hint)
    let cases = [
        (
            "horizontal split",
            SplitAxis::Horizontal,
            Size::new(Length::Fixed(size::SPLITTER_GRAB), Length::Fill),
        ),
        (
            "vertical split",
            SplitAxis::Vertical,
            Size::new(Length::Fill, Length::Fixed(size::SPLITTER_GRAB)),
        ),
    ];

    for (label, axis, expected_size) in cases {
        let divider: SplitDivider<'_, ()> = split_divider(SplitDividerModel::new(axis, 0.5));
        // `SplitDivider` is generic only over its message, so the trait's
        // `Theme`/`Renderer` parameters have to be spelled out by hand.
        let divider = &divider as &dyn Widget<(), iced::Theme, iced::Renderer>;

        assert_eq!(
            divider.tag(),
            iced::advanced::widget::tree::Tag::of::<SplitDividerState>(),
            "[{label}] the widget must claim its own state tag"
        );

        let state_hint = divider.state();
        let state = state_hint.downcast_ref::<SplitDividerState>();
        assert!(
            !state.resize.is_active(),
            "[{label}] a fresh widget must not be mid-drag"
        );
        assert!(
            !state.is_hovered,
            "[{label}] a fresh widget must not be hovered"
        );
        assert!(
            !state.is_focused,
            "[{label}] a fresh widget must not be focused"
        );

        assert_eq!(
            divider.size(),
            expected_size,
            "[{label}] the size hint must fix the divider's thickness and fill the other axis"
        );
    }
}

#[test]
fn test_widget_layout_resolves_the_divider_box_per_axis() {
    let renderer = headless_renderer();
    let limits = Limits::new(Size::ZERO, Size::new(400.0, 300.0));

    // (case label, axis, expected node size)
    let cases = [
        (
            "horizontal split",
            SplitAxis::Horizontal,
            Size::new(size::SPLITTER_GRAB, 300.0),
        ),
        (
            "vertical split",
            SplitAxis::Vertical,
            Size::new(400.0, size::SPLITTER_GRAB),
        ),
    ];

    for (label, axis, expected) in cases {
        let mut divider: SplitDivider<'_, ()> = split_divider(SplitDividerModel::new(axis, 0.5));
        let mut tree = Tree::new(&divider as &dyn Widget<(), iced::Theme, iced::Renderer>);
        let divider = &mut divider as &mut dyn Widget<(), iced::Theme, iced::Renderer>;
        let node = divider.layout(&mut tree, &renderer, &limits);

        assert_eq!(
            node.size(),
            expected,
            "[{label}] the divider must take its own thickness and fill the parent"
        );
        assert!(
            node.children().is_empty(),
            "[{label}] the divider lays out no children"
        );
    }
}

#[test]
fn test_the_divider_paints_its_stroke_and_grip_pill() {
    let palette = Palette::dark();
    let mut harness = Harness::<(), _>::recording(|| {
        split_divider(SplitDividerModel::new(SplitAxis::Horizontal, 0.5)).into()
    });

    let paints = harness.paint();
    assert_eq!(
        paints.len(),
        2,
        "the divider must paint its stroke and grip pill and nothing else"
    );

    // The harness viewport is 400x300, so a horizontal divider lays out to 10x300.
    assert_eq!(
        paints[0].bounds,
        Rectangle::new(Point::new(4.5, 0.0), Size::new(1.0, 300.0)),
        "the stroke must be the 1px line centred in the divider box"
    );
    assert_eq!(
        paints[0].background,
        Background::Color(palette.border_subtle),
        "the resting stroke must be painted in color.border.subtle"
    );
    assert_eq!(
        paints[1].bounds.size(),
        Size::new(space::XXS, size::HIT_TARGET_MIN),
        "the grip pill must use the resting pill size"
    );
    assert_eq!(
        paints[1].background,
        Background::Color(palette.border_strong),
        "the resting grip pill must be painted in color.border.strong"
    );
    assert_eq!(
        paints[1].border.radius,
        shape::RADIUS_PILL.into(),
        "the grip pill must be drawn with the pill radius"
    );
}

#[test]
fn test_a_disabled_divider_paints_a_muted_grip_pill() {
    let palette = Palette::dark();
    let mut harness = Harness::<(), _>::recording(|| {
        split_divider(SplitDividerModel::new(SplitAxis::Horizontal, 0.5).with_disabled(true)).into()
    });

    let paints = harness.paint();
    assert_eq!(
        paints.len(),
        2,
        "a disabled divider still shows where the split is"
    );
    assert_eq!(
        paints[1].background,
        Background::Color(palette.border_subtle),
        "a disabled grip pill must fall back to color.border.subtle"
    );
}

// ---------------------------------------------------------------------------
// Widget::update event handling
// ---------------------------------------------------------------------------

/// Host message emitted by the driven divider.
#[derive(Debug, Clone, PartialEq)]
enum TestMsg {
    /// Live ratio during a drag.
    Live(f32),
    /// Settled ratio.
    Commit(f32),
    /// Reverted pre-drag ratio.
    Cancel(f32),
}

/// Build a `KeyPressed` event for a named key.
fn key_event(named: Named) -> Event {
    let key = Key::Named(named);
    Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::default(),
        text: None,
        repeat: false,
    })
}

/// The horizontal divider box every event test drives: 10x300 at the origin,
/// so its grab band spans `x` 0..10.
fn horizontal_box() -> Rectangle {
    Rectangle::new(Point::ORIGIN, Size::new(size::SPLITTER_GRAB, 300.0))
}

/// The vertical divider box every event test drives: 600x10 at the origin,
/// so its grab band spans `y` 0..10.
fn vertical_box() -> Rectangle {
    Rectangle::new(Point::ORIGIN, Size::new(600.0, size::SPLITTER_GRAB))
}

/// Drives [`SplitDivider`] through the real `Widget::update` entry point so a
/// test can read the widget's internal state back out of its [`Tree`].
///
/// One renderer is built per driver and reused across every event the test
/// delivers, because building an iced renderer is the expensive part.
struct DividerDriver<'a> {
    widget: SplitDivider<'a, TestMsg>,
    tree: Tree,
    renderer: iced::Renderer,
    bounds: Rectangle,
    viewport: Rectangle,
    messages: Vec<TestMsg>,
}

impl<'a> DividerDriver<'a> {
    /// A divider with every resize callback attached, so a drag's live, commit
    /// and cancel payloads are observable from a test.
    fn wired(model: SplitDividerModel) -> SplitDivider<'a, TestMsg> {
        split_divider(model)
            .on_resize_change(|live| TestMsg::Live(*live.value()))
            .on_resize_commit(|commit| TestMsg::Commit(*commit.value()))
            .on_resize_cancel(|cancel| TestMsg::Cancel(cancel.restored().ratio))
    }

    /// Wrap a divider over `model` at `bounds`, with every callback attached.
    fn new(model: SplitDividerModel, bounds: Rectangle) -> Self {
        Self::with_widget(Self::wired(model), bounds)
    }

    /// Wrap an already-assembled divider, e.g. one with callbacks left off.
    fn with_widget(widget: SplitDivider<'a, TestMsg>, bounds: Rectangle) -> Self {
        let tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
        Self {
            widget,
            tree,
            renderer: headless_renderer(),
            bounds,
            viewport: bounds,
            messages: Vec::new(),
        }
    }

    /// A horizontal divider at the default box.
    fn horizontal(model: SplitDividerModel) -> Self {
        Self::new(model, horizontal_box())
    }

    /// A horizontal divider whose parent length is stated explicitly, so the
    /// ratio a given drag produces is readable.
    fn horizontal_with_length(model: SplitDividerModel, total_length: f32) -> Self {
        let widget = Self::wired(model).with_total_length(total_length);
        Self::with_widget(widget, horizontal_box())
    }

    /// A vertical divider at the default box.
    fn vertical(model: SplitDividerModel) -> Self {
        Self::new(model, vertical_box())
    }

    /// Deliver `event` with the pointer at `cursor`; report whether the
    /// divider claimed it.
    fn drive(&mut self, event: &Event, cursor: mouse::Cursor) -> bool {
        let Self {
            widget,
            tree,
            renderer,
            bounds,
            viewport,
            messages,
        } = self;

        let node = Node::new(bounds.size());
        let layout = Layout::new(&node);
        let mut clipboard = Null;
        let mut shell = Shell::new(messages);
        // `SplitDivider` is generic only over its message, so the trait's
        // `Theme`/`Renderer` parameters have to be spelled out by hand.
        let widget = widget as &mut dyn Widget<TestMsg, iced::Theme, iced::Renderer>;
        widget.update(
            tree,
            event,
            layout,
            cursor,
            renderer,
            &mut clipboard,
            &mut shell,
            viewport,
        );
        shell.is_event_captured()
    }

    /// Left-button press with the pointer at `at`.
    fn press(&mut self, at: Point) -> bool {
        self.drive(
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            mouse::Cursor::Available(at),
        )
    }

    /// Left-button release with the pointer at `at`.
    fn release(&mut self, at: Point) -> bool {
        self.drive(
            &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            mouse::Cursor::Available(at),
        )
    }

    /// Pointer move with the pointer at `at`.
    fn move_to(&mut self, at: Point) -> bool {
        self.drive(
            &Event::Mouse(mouse::Event::CursorMoved { position: at }),
            mouse::Cursor::Available(at),
        )
    }

    /// Escape, with no pointer position reported to the divider.
    fn escape(&mut self) -> bool {
        self.drive(&key_event(Named::Escape), mouse::Cursor::Unavailable)
    }

    /// Override the viewport rectangle delivered with each event.
    fn with_viewport(mut self, viewport: Rectangle) -> Self {
        self.viewport = viewport;
        self
    }

    /// The divider's live internal state.
    fn state(&self) -> &SplitDividerState {
        self.tree.state.downcast_ref::<SplitDividerState>()
    }

    /// Take every message emitted so far.
    fn take_messages(&mut self) -> Vec<TestMsg> {
        std::mem::take(&mut self.messages)
    }

    /// Ask the divider what cursor shape it wants for `cursor`.
    fn interaction(&self, cursor: mouse::Cursor) -> mouse::Interaction {
        let node = Node::new(self.bounds.size());
        let layout = Layout::new(&node);
        let widget = &self.widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>;
        widget.mouse_interaction(&self.tree, layout, cursor, &self.bounds, &self.renderer)
    }
}

#[test]
fn test_press_drag_and_release_inside_the_band_commits() {
    let mut driver = DividerDriver::horizontal_with_length(
        SplitDividerModel::new(SplitAxis::Horizontal, 0.5),
        64.0,
    );

    // A press inside the 10px band opens the gesture and claims the press.
    assert!(
        driver.press(Point::new(2.0, 150.0)),
        "a press on the band must be claimed"
    );
    assert!(
        driver.state().resize.is_active(),
        "a press on the band must open a gesture"
    );
    assert!(
        driver.state().is_hovered,
        "a press inside the band must read as hover"
    );
    assert!(
        driver.take_messages().is_empty(),
        "a press alone must emit nothing"
    );

    // 4px of travel over a 64px parent is a 0.0625 ratio step.
    assert!(
        driver.move_to(Point::new(6.0, 150.0)),
        "an open drag must claim the move"
    );
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::Live(0.5625)],
        "the drag must report the live ratio"
    );

    // Releasing inside the band settles the gesture at the live ratio.
    assert!(
        driver.release(Point::new(6.0, 150.0)),
        "a release must be claimed while a drag is open"
    );
    assert!(
        !driver.state().resize.is_active(),
        "a release must close the gesture"
    );
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::Commit(0.5625)],
        "releasing inside the band must commit the live ratio"
    );
}

#[test]
fn test_release_outside_the_band_cancels_the_drag() {
    let mut driver = DividerDriver::horizontal_with_length(
        SplitDividerModel::new(SplitAxis::Horizontal, 0.5),
        64.0,
    );

    assert!(driver.press(Point::new(5.0, 150.0)));

    // Drag far past the band: the ratio clamps at the ceiling.
    assert!(driver.move_to(Point::new(300.0, 150.0)));
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::Live(1.0)],
        "a drag past the ceiling must report the clamped ratio"
    );

    // Releasing outside the band reverts to the pre-drag snapshot.
    assert!(driver.release(Point::new(300.0, 150.0)));
    assert!(
        !driver.state().resize.is_active(),
        "a release must close the gesture"
    );
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::Cancel(0.5)],
        "releasing outside the band must cancel back to the pre-drag ratio"
    );
    assert_eq!(
        driver.state().resize.current_ratio(),
        0.5,
        "the gesture must rewind to its start ratio"
    );
}

#[test]
fn test_escape_cancels_an_active_drag() {
    let mut driver = DividerDriver::horizontal_with_length(
        SplitDividerModel::new(SplitAxis::Horizontal, 0.5),
        64.0,
    );

    assert!(driver.press(Point::new(2.0, 150.0)));
    assert!(driver.move_to(Point::new(6.0, 150.0)));
    assert_eq!(driver.take_messages(), vec![TestMsg::Live(0.5625)]);

    assert!(
        driver.escape(),
        "Escape must be claimed while a drag is open"
    );
    assert!(
        !driver.state().resize.is_active(),
        "Escape must close the gesture"
    );
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::Cancel(0.5)],
        "Escape must cancel back to the pre-drag ratio"
    );
    assert!(
        !driver.state().is_hovered,
        "Escape with no pointer reported must clear hover"
    );
}

#[test]
fn test_press_outside_the_band_is_not_claimed() {
    let mut driver = DividerDriver::horizontal(SplitDividerModel::new(SplitAxis::Horizontal, 0.5));

    assert!(
        !driver.press(Point::new(50.0, 150.0)),
        "a press beside the band must not be claimed"
    );
    assert!(
        !driver.state().resize.is_active(),
        "a press beside the band must not open a gesture"
    );
    assert!(
        !driver.state().is_hovered,
        "a pointer beside the band must not read as hover"
    );
    assert!(
        driver.take_messages().is_empty(),
        "an unclaimed press must emit nothing"
    );
}

#[test]
fn test_move_and_release_without_an_active_drag_are_not_claimed() {
    let mut driver = DividerDriver::horizontal(SplitDividerModel::new(SplitAxis::Horizontal, 0.5));

    assert!(
        !driver.move_to(Point::new(5.0, 150.0)),
        "a move with no open drag must not be claimed"
    );
    assert!(
        !driver.release(Point::new(5.0, 150.0)),
        "a release with no open drag must not be claimed"
    );
    assert!(
        !driver.escape(),
        "Escape with no open drag must not be claimed"
    );
    assert!(
        driver.take_messages().is_empty(),
        "an idle divider must emit nothing"
    );
    assert!(!driver.state().resize.is_active());
}

#[test]
fn test_hover_tracks_the_band_and_freezes_during_a_drag() {
    let mut driver = DividerDriver::horizontal_with_length(
        SplitDividerModel::new(SplitAxis::Horizontal, 0.5),
        64.0,
    );

    driver.move_to(Point::new(5.0, 150.0));
    assert!(
        driver.state().is_hovered,
        "a pointer on the band must read as hover"
    );

    driver.move_to(Point::new(50.0, 150.0));
    assert!(
        !driver.state().is_hovered,
        "a pointer off the band must clear hover"
    );

    // Press, then drag off the band: hover stays latched for the drag.
    assert!(driver.press(Point::new(5.0, 150.0)));
    assert!(driver.state().is_hovered);
    driver.move_to(Point::new(300.0, 150.0));
    assert!(
        driver.state().is_hovered,
        "hover must stay latched for the duration of a drag"
    );

    // Releasing off the band cancels and re-reads the pointer position.
    driver.release(Point::new(300.0, 150.0));
    assert!(
        !driver.state().is_hovered,
        "a release off the band must clear the latched hover"
    );
}

#[test]
fn test_a_disabled_divider_refuses_the_drag() {
    let model = SplitDividerModel::new(SplitAxis::Horizontal, 0.5).with_disabled(true);
    let mut driver = DividerDriver::horizontal(model);

    assert!(
        !driver.press(Point::new(5.0, 150.0)),
        "a disabled divider must not claim a press on its own band"
    );
    assert!(
        !driver.state().resize.is_active(),
        "a disabled divider must not open a gesture"
    );
    assert!(
        driver.take_messages().is_empty(),
        "a disabled divider must emit nothing"
    );
    assert_eq!(
        driver.interaction(mouse::Cursor::Available(Point::new(5.0, 150.0))),
        mouse::Interaction::None,
        "a disabled divider must not offer a resize cursor"
    );
}

#[test]
fn test_a_drag_without_callbacks_still_drives_the_gesture() {
    let widget =
        split_divider(SplitDividerModel::new(SplitAxis::Horizontal, 0.5)).with_total_length(64.0);
    let mut driver = DividerDriver::with_widget(widget, horizontal_box());

    assert!(
        driver.press(Point::new(2.0, 150.0)),
        "the press is still claimed"
    );
    assert!(
        driver.move_to(Point::new(6.0, 150.0)),
        "the move is still claimed"
    );
    assert!(
        driver.take_messages().is_empty(),
        "with no callbacks attached the drag must emit nothing"
    );
    assert_eq!(
        driver.state().resize.current_ratio(),
        0.5625,
        "the gesture must still track the pointer with no callbacks attached"
    );

    assert!(driver.release(Point::new(6.0, 150.0)));
    assert!(
        !driver.state().resize.is_active(),
        "the release must still close the gesture"
    );
    assert!(
        driver.take_messages().is_empty(),
        "a commit with no callback attached must emit nothing"
    );
}

#[test]
fn test_total_length_falls_back_to_the_viewport() {
    // With no `with_total_length`, a horizontal divider reads the viewport
    // width — which for a divider box is the divider's own 10px thickness.
    let mut horizontal =
        DividerDriver::horizontal(SplitDividerModel::new(SplitAxis::Horizontal, 0.5));
    assert!(horizontal.press(Point::new(2.0, 150.0)));
    assert!(horizontal.move_to(Point::new(7.0, 150.0)));
    assert_eq!(
        horizontal.take_messages(),
        vec![TestMsg::Live(1.0)],
        "a 5px move over a 10px fallback length must saturate the ratio"
    );

    // A vertical divider reads the viewport height instead.
    let mut vertical = DividerDriver::vertical(SplitDividerModel::new(SplitAxis::Vertical, 0.5));
    assert!(vertical.press(Point::new(300.0, 2.0)));
    assert!(vertical.move_to(Point::new(300.0, 7.0)));
    assert_eq!(
        vertical.take_messages(),
        vec![TestMsg::Live(1.0)],
        "a 5px move over a 10px fallback height must saturate the ratio"
    );
}

#[test]
fn test_a_zero_sized_viewport_freezes_the_ratio() {
    let mut driver = DividerDriver::horizontal(SplitDividerModel::new(SplitAxis::Horizontal, 0.5))
        .with_viewport(Rectangle::new(Point::ORIGIN, Size::ZERO));

    assert!(
        driver.press(Point::new(5.0, 150.0)),
        "a zero viewport must not stop the press opening the gesture"
    );
    assert!(
        driver.move_to(Point::new(50.0, 150.0)),
        "the move is still claimed while the drag is open"
    );
    assert!(
        driver.take_messages().is_empty(),
        "a zero-length viewport must produce no live ratio"
    );
    assert_eq!(
        driver.state().resize.current_ratio(),
        0.5,
        "a zero-length viewport must leave the ratio untouched"
    );
}

#[test]
fn test_mouse_interaction_per_axis_and_state() {
    // (case label, axis, cursor position, expected interaction)
    let cases = [
        (
            "on the horizontal band",
            SplitAxis::Horizontal,
            Point::new(5.0, 150.0),
            mouse::Interaction::ResizingHorizontally,
        ),
        (
            "beside the horizontal band",
            SplitAxis::Horizontal,
            Point::new(50.0, 150.0),
            mouse::Interaction::None,
        ),
        (
            "on the vertical band",
            SplitAxis::Vertical,
            Point::new(300.0, 5.0),
            mouse::Interaction::ResizingVertically,
        ),
        (
            "beside the vertical band",
            SplitAxis::Vertical,
            Point::new(300.0, 200.0),
            mouse::Interaction::None,
        ),
    ];

    for (label, axis, at, expected) in cases {
        let model = SplitDividerModel::new(axis, 0.5);
        let driver = match axis {
            SplitAxis::Horizontal => DividerDriver::horizontal(model),
            SplitAxis::Vertical => DividerDriver::vertical(model),
        };

        assert_eq!(
            driver.interaction(mouse::Cursor::Available(at)),
            expected,
            "[{label}] the cursor shape must follow the axis"
        );
        assert_eq!(
            driver.interaction(mouse::Cursor::Unavailable),
            mouse::Interaction::None,
            "[{label}] no pointer must mean no resize cursor"
        );
    }

    // While a drag is open the resize cursor stays offered even with the
    // pointer well outside the band.
    let mut driver = DividerDriver::horizontal_with_length(
        SplitDividerModel::new(SplitAxis::Horizontal, 0.5),
        64.0,
    );
    assert!(driver.press(Point::new(5.0, 150.0)));
    assert_eq!(
        driver.interaction(mouse::Cursor::Available(Point::new(500.0, 150.0))),
        mouse::Interaction::ResizingHorizontally,
        "an open drag must keep offering the resize cursor"
    );
}
