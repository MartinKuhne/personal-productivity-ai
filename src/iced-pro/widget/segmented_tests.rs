//! Tests for the segmented-control resolver and geometry —
//! `UIF-ATOM-001b`, §3.1b.
//!
//! Seven groups, in order:
//!
//! 1. **Composition.** The unselected segment *is* the Secondary button
//!    resolver's answer, and the selected segment *is* its `Pressed`
//!    cell. These are the tests that make this atom a composition rather
//!    than a copy.
//! 2. **The held paint.** Selection does not move under hover or press,
//!    and the collision with a pressed unselected segment is asserted as
//!    the finding it is.
//! 3. **The check glyph.** Its colour by state, and when it is painted.
//! 4. **Focus.** The ring is the strip's, resolved against the mount.
//! 5. **Geometry.** The reserved slot is charged to every segment, and
//!    the boxes tile the strip without overlap or gap.
//! 6. **Contrast.** Every ratio the module doc quotes, recomputed from
//!    the raw sRGB through the crate's own `contrast_ratio`.
//! 7. **Live widget.** Controlled creation, hover, clicks, selection mode
//!    transitions, disabled gating, keyboard navigation, and headless rendering.

use std::collections::BTreeSet;

use super::*;
use crate::state::BaseState;
use crate::test_helpers::{Harness, focus_one};
use crate::token::{contrast, contrast::contrast_ratio};
use crate::widget::button_model::ButtonDensity;
use crate::widget::segmented_model::{SegmentItem, SelectionMode};
use iced::advanced::widget::Widget;
use iced::keyboard::key::{Key, Named};
use iced_test::renderer::Renderer as HeadlessRenderer;

fn palette() -> Palette {
    Palette::dark()
}

const REST: VisualState = VisualState {
    base: BaseState::Rest,
    focused: false,
};
const HOVERED: VisualState = VisualState {
    base: BaseState::Hovered,
    focused: false,
};
const PRESSED: VisualState = VisualState {
    base: BaseState::Pressed,
    focused: false,
};
const DISABLED: VisualState = VisualState {
    base: BaseState::Disabled,
    focused: false,
};
const REST_FOCUSED: VisualState = VisualState {
    base: BaseState::Rest,
    focused: true,
};

/// Every base state the resolver is asked about, in spec column order.
const BASES: [BaseState; 5] = [
    BaseState::Rest,
    BaseState::Hovered,
    BaseState::Pressed,
    BaseState::Invalid,
    BaseState::Disabled,
];

// ---------------------------------------------------------------------------
// 1. Composition
// ---------------------------------------------------------------------------

#[test]
fn an_unselected_segment_is_the_secondary_button_style_verbatim() {
    let p = palette();
    for base in BASES {
        let state = VisualState {
            base,
            focused: false,
        };
        let segment = resolve_style(SegmentPolarity::Unselected, state, &p);
        let button = button::resolve_style(ButtonVariant::Secondary, state, &p);
        assert_eq!(
            segment.paint, button,
            "the unselected segment must not differ from the button atom in any channel at {base:?}"
        );
    }
}

#[test]
fn a_selected_segment_is_the_secondary_pressed_cell() {
    let p = palette();
    let expected = button::resolve_style(
        ButtonVariant::Secondary,
        VisualState {
            base: BaseState::Pressed,
            focused: false,
        },
        &p,
    );
    for base in [BaseState::Rest, BaseState::Hovered, BaseState::Pressed] {
        let segment = resolve_style(
            SegmentPolarity::Selected,
            VisualState {
                base,
                focused: false,
            },
            &p,
        );
        assert_eq!(
            segment.paint, expected,
            "the selected segment must be the Secondary Pressed cell at {base:?}"
        );
    }
}

#[test]
fn a_selected_segment_is_the_secondary_disabled_cell_when_disabled() {
    let p = palette();
    let expected = button::resolve_style(
        ButtonVariant::Secondary,
        VisualState {
            base: BaseState::Disabled,
            focused: false,
        },
        &p,
    );
    let segment = resolve_style(SegmentPolarity::Selected, DISABLED, &p);
    assert_eq!(segment.paint, expected);
    assert_eq!(segment.paint.background, p.surface_base);
    assert_eq!(segment.paint.foreground, p.text_disabled);
    assert_eq!(segment.paint.border_color, p.border_subtle);
}

#[test]
fn the_selected_fill_is_surface_active_and_its_border_is_the_accent() {
    let p = palette();
    let segment = resolve_style(SegmentPolarity::Selected, REST, &p);
    assert_eq!(segment.paint.background, p.surface_active);
    assert_eq!(segment.paint.foreground, p.text_strong);
    assert_eq!(segment.paint.border_color, p.accent_border);
}

// ---------------------------------------------------------------------------
// 2. The held paint, and the collision it creates
// ---------------------------------------------------------------------------

#[test]
fn the_selected_paint_does_not_move_under_hover_or_press() {
    let p = palette();
    let at_rest = resolve_style(SegmentPolarity::Selected, REST, &p).paint;
    let hovered = resolve_style(SegmentPolarity::Selected, HOVERED, &p).paint;
    let pressed = resolve_style(SegmentPolarity::Selected, PRESSED, &p).paint;
    assert_eq!(at_rest, hovered, "hover must not darken the selection");
    assert_eq!(at_rest, pressed, "press must not move the selection either");
}

#[test]
fn a_pressed_unselected_segment_collides_with_a_selected_one() {
    let p = palette();
    let selected = resolve_style(SegmentPolarity::Selected, REST, &p).paint;
    let pressed_unselected = resolve_style(SegmentPolarity::Unselected, PRESSED, &p).paint;

    assert_eq!(
        selected.background, pressed_unselected.background,
        "FINDING: pressed-unselected and selected share the fill"
    );
    assert_eq!(
        selected.foreground, pressed_unselected.foreground,
        "FINDING: pressed-unselected and selected share the foreground"
    );
    assert_eq!(
        selected.border_color, pressed_unselected.border_color,
        "FINDING: pressed-unselected and selected share the border"
    );

    let selected_check = resolve_style(SegmentPolarity::Selected, REST, &p);
    let pressed_check = resolve_style(SegmentPolarity::Unselected, PRESSED, &p);
    assert!(
        selected_check.shows_check && !pressed_check.shows_check,
        "the reserved ✓ is the ONLY thing that tells them apart"
    );
}

#[test]
fn a_resting_unselected_segment_is_distinguishable_from_a_selected_one() {
    let p = palette();
    let selected = resolve_style(SegmentPolarity::Selected, REST, &p).paint;
    let resting = resolve_style(SegmentPolarity::Unselected, REST, &p).paint;
    assert_ne!(selected.background, resting.background);
    assert_ne!(selected.border_color, resting.border_color);
}

// ---------------------------------------------------------------------------
// 3. The check glyph
// ---------------------------------------------------------------------------

#[test]
fn the_check_is_the_accent_text_token_while_live() {
    let p = palette();
    for base in [BaseState::Rest, BaseState::Hovered, BaseState::Pressed] {
        let style = resolve_style(
            SegmentPolarity::Selected,
            VisualState {
                base,
                focused: false,
            },
            &p,
        );
        assert_eq!(style.check, p.accent_text, "at {base:?}");
        assert!(style.shows_check);
    }
}

#[test]
fn the_check_goes_disabled_with_the_strip_but_stays_painted() {
    let p = palette();
    let style = resolve_style(SegmentPolarity::Selected, DISABLED, &p);
    assert_eq!(style.check, p.text_disabled);
    assert!(
        style.shows_check,
        "a disabled selected segment keeps its ✓ rather than un-checking itself"
    );
}

#[test]
fn an_unselected_segment_never_paints_the_check() {
    let p = palette();
    for base in BASES {
        let style = resolve_style(
            SegmentPolarity::Unselected,
            VisualState {
                base,
                focused: false,
            },
            &p,
        );
        assert!(!style.shows_check, "at {base:?}");
    }
}

// ---------------------------------------------------------------------------
// 4. Focus
// ---------------------------------------------------------------------------

#[test]
fn focus_adds_a_ring_without_touching_the_selected_paint() {
    let p = palette();
    let plain = resolve_style(SegmentPolarity::Selected, REST, &p).paint;
    let focused = resolve_style(
        SegmentPolarity::Selected,
        VisualState {
            base: BaseState::Rest,
            focused: true,
        },
        &p,
    )
    .paint;
    assert_eq!(focused.background, plain.background);
    assert_eq!(focused.foreground, plain.foreground);
    assert_eq!(focused.border_color, plain.border_color);
    assert!(focused.focus_ring_width > 0.0);
}

#[test]
fn the_strip_ring_is_resolved_against_the_mount_not_the_segment() {
    let p = palette();
    let on_base = resolve_strip_ring(REST_FOCUSED, p.surface_base, &p).expect("a ring");
    let on_raised = resolve_strip_ring(REST_FOCUSED, p.surface_raised, &p).expect("a ring");
    assert_eq!(on_base.1, FOCUS_RING_WIDTH);
    assert_eq!(on_raised.1, FOCUS_RING_WIDTH);
    assert_eq!(on_base.0, p.accent_border);
    assert_eq!(on_raised.0, p.accent_border);
}

#[test]
fn no_ring_is_offered_for_an_unfocused_or_disabled_state() {
    let p = palette();
    assert!(resolve_strip_ring(REST, p.surface_base, &p).is_none());
    assert!(resolve_strip_ring(DISABLED, p.surface_base, &p).is_none());
}

// ---------------------------------------------------------------------------
// 5. Geometry
// ---------------------------------------------------------------------------

#[test]
fn every_segment_pays_for_the_check_slot_whether_or_not_it_shows_one() {
    let label = 60.0;
    let width = segment_width(label);
    assert_eq!(
        width,
        label + CHECK_SLOT_WIDTH + SEGMENT_PADDING * 2.0,
        "the reserved slot is in the width unconditionally"
    );
    assert!(width > label + SEGMENT_PADDING * 2.0);
}

#[test]
fn the_strip_width_is_the_sum_of_its_segments() {
    let labels = [40.0, 60.0, 25.0];
    assert_eq!(
        strip_width(&labels),
        segment_width(40.0) + segment_width(60.0) + segment_width(25.0)
    );
}

#[test]
fn an_empty_strip_is_zero_wide() {
    assert_eq!(strip_width(&[]), 0.0);
}

#[test]
fn a_negative_label_width_is_clamped_not_subtracted() {
    let width = segment_width(-100.0);
    assert_eq!(width, CHECK_SLOT_WIDTH + SEGMENT_PADDING * 2.0);
}

#[test]
fn segment_bounds_tile_the_strip_with_no_gap_and_no_overlap() {
    let labels = [40.0, 60.0, 25.0];
    let strip = Rectangle::new(
        Point::ORIGIN,
        Size::new(strip_width(&labels), HEIGHT_STANDARD),
    );

    let mut cursor_x = 0.0;
    for (index, label) in labels.iter().enumerate() {
        let bounds = segment_bounds(strip, index, &labels);
        assert_eq!(
            bounds.x, cursor_x,
            "segment {index} starts where the last ended"
        );
        assert_eq!(bounds.width, segment_width(*label));
        assert_eq!(bounds.y, 0.0);
        assert_eq!(bounds.height, HEIGHT_STANDARD);
        cursor_x += segment_width(*label);
    }
    assert_eq!(cursor_x, strip.width, "the tiles exactly cover the strip");
}

#[test]
fn a_segment_index_past_the_end_is_zero_wide_at_the_strip_end() {
    let labels = [40.0, 60.0];
    let strip = Rectangle::new(
        Point::ORIGIN,
        Size::new(strip_width(&labels), HEIGHT_COMPACT),
    );
    let bounds = segment_bounds(strip, 5, &labels);
    assert_eq!(bounds.width, 0.0);
    assert_eq!(bounds.x, strip.width);
}

#[test]
fn the_check_slot_sits_inside_its_segment_at_the_leading_padding() {
    let labels = [60.0];
    let strip = Rectangle::new(
        Point::ORIGIN,
        Size::new(strip_width(&labels), HEIGHT_STANDARD),
    );
    let segment = segment_bounds(strip, 0, &labels);
    let slot = check_slot(segment);

    assert_eq!(slot.x, segment.x + SEGMENT_PADDING);
    assert_eq!(slot.width, CHECK_GLYPH);
    assert_eq!(slot.height, CHECK_GLYPH);
    assert!(
        slot.y > segment.y,
        "the glyph is centred, not glued to the top"
    );
    assert!(slot.y + slot.height < segment.y + segment.height);
    assert!(slot.x + slot.width <= segment.x + segment.width);
}

#[test]
fn the_label_bounds_start_past_the_reserved_slot() {
    let labels = [60.0];
    let strip = Rectangle::new(
        Point::ORIGIN,
        Size::new(strip_width(&labels), HEIGHT_STANDARD),
    );
    let segment = segment_bounds(strip, 0, &labels);
    let label = label_bounds(segment);
    let slot = check_slot(segment);

    assert_eq!(label.x, slot.x + slot.width + CHECK_GAP);
    assert_eq!(label.width, 60.0);
    assert!(
        label.x >= slot.x + slot.width,
        "the label never overlaps the ✓"
    );
}

#[test]
fn selection_changing_moves_no_label_box() {
    let labels = [40.0, 60.0];
    let strip = Rectangle::new(
        Point::ORIGIN,
        Size::new(strip_width(&labels), HEIGHT_STANDARD),
    );
    for index in 0..2 {
        let segment = segment_bounds(strip, index, &labels);
        let selected_label = label_bounds(segment);
        let unselected_label = label_bounds(segment);
        assert_eq!(
            selected_label, unselected_label,
            "the label box is a function of geometry only, never of selection"
        );
    }
}

// ---------------------------------------------------------------------------
// 6. Contrast — recomputed from the raw sRGB
// ---------------------------------------------------------------------------

#[test]
fn the_selected_label_ratio_matches_what_the_spec_prints() {
    let p = palette();
    let ratio = contrast_ratio(p.text_strong, p.surface_active);
    assert!(
        (ratio - 14.2946).abs() < 1e-3,
        "text.strong on surface.active is {ratio}, spec prints 14.29"
    );
    assert!(ratio >= contrast::MIN_TEXT_CONTRAST);
}

#[test]
fn the_selected_border_ratio_matches_what_the_spec_prints() {
    let p = palette();
    let ratio = contrast_ratio(p.accent_border, p.surface_active);
    assert!(
        (ratio - 4.7922).abs() < 1e-3,
        "accent.border on surface.active is {ratio}, spec prints 4.79"
    );
    assert!(ratio >= contrast::MIN_CHROME_CONTRAST);
}

#[test]
fn the_check_glyph_ratio_matches_what_the_spec_prints() {
    let p = palette();
    let ratio = contrast_ratio(p.accent_text, p.surface_active);
    assert!(
        (ratio - 7.6562).abs() < 1e-3,
        "accent.text on surface.active is {ratio}, spec prints 7.66"
    );
    assert!(ratio >= contrast::MIN_CHROME_CONTRAST);
}

#[test]
fn the_disabled_selected_label_sits_inside_the_disabled_band() {
    let p = palette();
    let ratio = contrast_ratio(p.text_disabled, p.surface_base);
    assert!(
        (ratio - 5.9165).abs() < 1e-3,
        "text.disabled on surface.base is {ratio}"
    );
    assert!(ratio >= contrast::DISABLED_TEXT_MIN_CONTRAST);
    assert!(ratio <= contrast::DISABLED_TEXT_MAX_CONTRAST);
}

#[test]
fn the_perimeter_is_judged_chrome_in_every_live_state() {
    let p = palette();
    for base in [BaseState::Rest, BaseState::Hovered, BaseState::Pressed] {
        let style = resolve_style(
            SegmentPolarity::Selected,
            VisualState {
                base,
                focused: false,
            },
            &p,
        );
        assert_eq!(
            perimeter_class(&style, &p),
            ChromeClass::Judged,
            "at {base:?}"
        );
    }
}

#[test]
fn the_disabled_perimeter_is_a_resting_divider_and_exempt() {
    let p = palette();
    let style = resolve_style(SegmentPolarity::Selected, DISABLED, &p);
    assert_eq!(
        perimeter_class(&style, &p),
        ChromeClass::RestingDivider,
        "border.subtle is not indicator material"
    );
}

#[test]
fn the_strip_heights_are_the_two_spec_rungs() {
    assert_eq!(HEIGHT_COMPACT, 28.0);
    assert_eq!(HEIGHT_STANDARD, 32.0);
    assert_eq!(RADIUS, 4.0);
    assert_eq!(PERIMETER_WIDTH, 1.0);
    assert_eq!(FOCUS_RING_WIDTH, 2.0);
    assert_eq!(CHECK_GLYPH, 14.0);
    assert_eq!(CHECK_SLOT_WIDTH, 22.0);
}

// ---------------------------------------------------------------------------
// 7. Live widget
// ---------------------------------------------------------------------------

fn leaked<Id: Clone + Ord>(model: SegmentedModel<Id>) -> &'static SegmentedModel<Id> {
    Box::leak(Box::new(model))
}

fn key_press(key: Named) -> Event {
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: Key::Named(key),
        modified_key: Key::Named(key),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::NONE,
        text: None,
        repeat: false,
    })
}

fn widget_bounds<M: Send + 'static>(harness: &mut Harness<M, HeadlessRenderer>) -> Rectangle {
    let recorded = harness.recorded();
    assert!(
        !recorded.is_empty(),
        "a segmented button registers exactly one focusable node, got {recorded:?}"
    );
    recorded[0].1
}

fn sample_segments() -> Vec<SegmentItem<&'static str>> {
    vec![
        SegmentItem::new("left", "Left"),
        SegmentItem::new("center", "Center"),
        SegmentItem::new("right", "Right"),
    ]
}

#[test]
fn test_controlled_creation_and_state_initialization() {
    let model = leaked(SegmentedModel::new(sample_segments()));
    let btn = SegmentedButton::<BTreeSet<&'static str>, &'static str>::controlled(
        Controlled::new(model),
        "seg_btn",
        Some,
    );

    assert_eq!(btn.focus_id().as_str(), "seg_btn");
    assert_eq!(btn.widget_id(), &widget_id(btn.focus_id()));
    assert_eq!(btn.model().segments.len(), 3);
    assert_eq!(btn.palette_ref(), &Palette::dark());

    let mut custom_palette = Palette::dark();
    custom_palette.accent_text = Color::WHITE;
    let btn_custom = btn.with_palette(custom_palette);
    assert_eq!(btn_custom.palette_ref(), &custom_palette);

    let tag = Widget::<BTreeSet<&'static str>, iced::Theme, HeadlessRenderer>::tag(&btn_custom);
    assert_eq!(tag, tree::Tag::of::<SegmentedState>());

    let state = Widget::<BTreeSet<&'static str>, iced::Theme, HeadlessRenderer>::state(&btn_custom);
    let downcast = state.downcast_ref::<SegmentedState>();
    assert_eq!(downcast.hovered, None);
    assert_eq!(downcast.pressed, None);
    assert_eq!(downcast.focused_segment, None);
    assert!(!downcast.focusable.is_focused());

    let size = Widget::<BTreeSet<&'static str>, iced::Theme, HeadlessRenderer>::size(&btn_custom);
    assert_eq!(
        size,
        Size::new(Length::Shrink, Length::Fixed(HEIGHT_STANDARD))
    );
}

#[test]
fn test_mouse_hover_over_different_segments() {
    let model = leaked(SegmentedModel::new(sample_segments()));
    let mut harness = Harness::headless(move || {
        SegmentedButton::controlled(Controlled::new(model), "seg_hover", Some).into()
    });

    let bounds = widget_bounds(&mut harness);
    let label_widths = [
        "Left".len() as f32 * 8.0,
        "Center".len() as f32 * 8.0,
        "Right".len() as f32 * 8.0,
    ];

    let b0 = segment_bounds(bounds, 0, &label_widths);
    let b1 = segment_bounds(bounds, 1, &label_widths);
    let b2 = segment_bounds(bounds, 2, &label_widths);

    let pt0 = Point::new(b0.x + b0.width / 2.0, b0.y + b0.height / 2.0);
    let pt1 = Point::new(b1.x + b1.width / 2.0, b1.y + b1.height / 2.0);
    let pt2 = Point::new(b2.x + b2.width / 2.0, b2.y + b2.height / 2.0);
    let outside_pt = Point::new(bounds.x + bounds.width + 50.0, bounds.y + 50.0);

    // Hover emits no messages
    let msgs0 = harness.hover(pt0);
    assert!(msgs0.is_empty());

    let msgs1 = harness.hover(pt1);
    assert!(msgs1.is_empty());

    let msgs2 = harness.hover(pt2);
    assert!(msgs2.is_empty());

    let msgs_out = harness.hover(outside_pt);
    assert!(msgs_out.is_empty());
}

#[test]
fn test_click_unselected_segment_toggles_selection_single_select() {
    let model = leaked(SegmentedModel::new(sample_segments()).with_selected("left"));
    let mut harness = Harness::headless(move || {
        SegmentedButton::controlled(Controlled::new(model), "seg_click_single", Some).into()
    });

    let bounds = widget_bounds(&mut harness);
    let label_widths = [
        "Left".len() as f32 * 8.0,
        "Center".len() as f32 * 8.0,
        "Right".len() as f32 * 8.0,
    ];

    let b1 = segment_bounds(bounds, 1, &label_widths);
    let pt1 = Point::new(b1.x + b1.width / 2.0, b1.y + b1.height / 2.0);

    harness.hover(pt1);
    let press_msgs = harness.press(pt1);
    assert!(
        press_msgs.is_empty(),
        "press on segment produces no immediate emission"
    );

    let release_msgs = harness.release(pt1);
    assert_eq!(release_msgs.len(), 1);
    assert_eq!(release_msgs[0], BTreeSet::from(["center"]));

    // Drag out cancels
    harness.hover(pt1);
    harness.press(pt1);
    let cancel_msgs = harness.release(Point::new(bounds.x + 1000.0, bounds.y + 1000.0));
    assert!(cancel_msgs.is_empty(), "drag out produces no emission");

    // Click on already selected segment produces no emission in SingleSelect mode
    let b0 = segment_bounds(bounds, 0, &label_widths);
    let pt0 = Point::new(b0.x + b0.width / 2.0, b0.y + b0.height / 2.0);
    harness.hover(pt0);
    harness.press(pt0);
    let no_evict_msgs = harness.release(pt0);
    assert!(
        no_evict_msgs.is_empty(),
        "clicking held single-select member is a no-op"
    );
}

#[test]
fn test_click_unselected_segment_toggles_selection_multi_select() {
    let model = leaked(SegmentedModel::multi(sample_segments()).with_selected("left"));
    let mut harness = Harness::headless(move || {
        SegmentedButton::controlled(Controlled::new(model), "seg_click_multi", Some).into()
    });

    let bounds = widget_bounds(&mut harness);
    let label_widths = [
        "Left".len() as f32 * 8.0,
        "Center".len() as f32 * 8.0,
        "Right".len() as f32 * 8.0,
    ];

    let b1 = segment_bounds(bounds, 1, &label_widths);
    let pt1 = Point::new(b1.x + b1.width / 2.0, b1.y + b1.height / 2.0);

    harness.hover(pt1);
    let press_msgs = harness.press(pt1);
    assert!(press_msgs.is_empty());

    let add_msgs = harness.release(pt1);
    assert_eq!(add_msgs.len(), 1);
    assert_eq!(add_msgs[0], BTreeSet::from(["left", "center"]));

    // Click on already selected segment removes it in MultiSelect mode
    let b0 = segment_bounds(bounds, 0, &label_widths);
    let pt0 = Point::new(b0.x + b0.width / 2.0, b0.y + b0.height / 2.0);
    harness.hover(pt0);
    harness.press(pt0);
    let remove_msgs = harness.release(pt0);
    assert_eq!(remove_msgs.len(), 1);
    assert_eq!(remove_msgs[0], BTreeSet::new());
}

#[test]
fn test_click_disabled_segment_produces_no_emission() {
    let segments = vec![
        SegmentItem::new("a", "Alpha"),
        SegmentItem::new("b", "Beta").disabled(),
        SegmentItem::new("c", "Gamma"),
    ];
    let model = leaked(SegmentedModel::new(segments));
    let mut harness = Harness::headless(move || {
        SegmentedButton::controlled(Controlled::new(model), "seg_disabled_item", Some).into()
    });

    let bounds = widget_bounds(&mut harness);
    let label_widths = [
        "Alpha".len() as f32 * 8.0,
        "Beta".len() as f32 * 8.0,
        "Gamma".len() as f32 * 8.0,
    ];

    let b1 = segment_bounds(bounds, 1, &label_widths);
    let pt1 = Point::new(b1.x + b1.width / 2.0, b1.y + b1.height / 2.0);

    harness.hover(pt1);
    assert!(harness.press(pt1).is_empty());
    assert!(harness.release(pt1).is_empty());

    // Whole strip disabled
    let inert_model = leaked(SegmentedModel::new(sample_segments()).disabled());
    let mut inert_harness = Harness::headless(move || {
        SegmentedButton::controlled(Controlled::new(inert_model), "seg_inert", Some).into()
    });

    let b0 = Point::new(10.0, 10.0);
    inert_harness.hover(b0);
    assert!(inert_harness.press(b0).is_empty());
    assert!(inert_harness.release(b0).is_empty());
}

#[test]
fn test_keyboard_arrow_roving_and_selection() {
    let model = leaked(SegmentedModel::new(sample_segments()));
    let mut harness = Harness::headless(move || {
        SegmentedButton::controlled(Controlled::new(model), "seg_kb", Some).into()
    });

    focus_one(&mut harness, "seg_kb");

    // ArrowRight roves to first enabled segment (0, "left")
    let r1 = harness.pointer(&[key_press(Named::ArrowRight)], mouse::Cursor::Unavailable);
    assert!(r1.is_empty());

    // ArrowRight roves to next enabled segment (1, "center")
    let r2 = harness.pointer(&[key_press(Named::ArrowRight)], mouse::Cursor::Unavailable);
    assert!(r2.is_empty());

    // Space selects focused segment ("center")
    let space_msgs = harness.pointer(&[key_press(Named::Space)], mouse::Cursor::Unavailable);
    assert_eq!(space_msgs.len(), 1);
    assert_eq!(space_msgs[0], BTreeSet::from(["center"]));

    // ArrowLeft roves backward to segment (0, "left")
    let l1 = harness.pointer(&[key_press(Named::ArrowLeft)], mouse::Cursor::Unavailable);
    assert!(l1.is_empty());

    // Enter selects focused segment ("left")
    let enter_msgs = harness.pointer(&[key_press(Named::Enter)], mouse::Cursor::Unavailable);
    assert_eq!(enter_msgs.len(), 1);
    assert_eq!(enter_msgs[0], BTreeSet::from(["left"]));
}

#[test]
fn test_segment_radius_combinations() {
    let single = segment_radius(0, 1);
    assert_eq!(single.top_left, RADIUS);
    assert_eq!(single.top_right, RADIUS);
    assert_eq!(single.bottom_right, RADIUS);
    assert_eq!(single.bottom_left, RADIUS);

    let first_of_three = segment_radius(0, 3);
    assert_eq!(first_of_three.top_left, RADIUS);
    assert_eq!(first_of_three.bottom_left, RADIUS);
    assert_eq!(first_of_three.top_right, 0.0);
    assert_eq!(first_of_three.bottom_right, 0.0);

    let middle_of_three = segment_radius(1, 3);
    assert_eq!(middle_of_three.top_left, 0.0);
    assert_eq!(middle_of_three.bottom_left, 0.0);
    assert_eq!(middle_of_three.top_right, 0.0);
    assert_eq!(middle_of_three.bottom_right, 0.0);

    let last_of_three = segment_radius(2, 3);
    assert_eq!(last_of_three.top_left, 0.0);
    assert_eq!(last_of_three.bottom_left, 0.0);
    assert_eq!(last_of_three.top_right, RADIUS);
    assert_eq!(last_of_three.bottom_right, RADIUS);
}

#[test]
fn test_headless_draw_across_states() {
    let p = palette();
    for density in [ButtonDensity::Standard, ButtonDensity::Compact] {
        for mode in [SelectionMode::SingleSelect, SelectionMode::MultiSelect] {
            for disabled in [false, true] {
                let mut m = SegmentedModel::new(sample_segments())
                    .with_mode(mode)
                    .with_density(density)
                    .with_selected("left");
                if disabled {
                    m = m.disabled();
                }
                let model = leaked(m);
                let mut harness = Harness::headless(move || {
                    SegmentedButton::controlled(Controlled::new(model), "seg_render", Some)
                        .palette(p)
                        .into()
                });

                let pt0 = Point::new(15.0, 10.0);
                let pt1 = Point::new(60.0, 10.0);

                // Rest render
                harness.render();

                // Hover segment 0
                harness.hover(pt0);
                harness.render();

                // Press segment 0
                harness.press(pt0);
                harness.render();
                harness.release(pt0);

                // Hover segment 1
                harness.hover(pt1);
                harness.render();

                // Press segment 1
                harness.press(pt1);
                harness.render();
                harness.release(pt1);

                if !disabled {
                    focus_one(&mut harness, "seg_render");
                    harness.render();
                }
            }
        }
    }
}
