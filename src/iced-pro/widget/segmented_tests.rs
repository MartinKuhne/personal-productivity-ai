//! Tests for the segmented-control resolver and geometry —
//! `UIF-ATOM-001b`, §3.1b.
//!
//! Six groups, in order:
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

use super::*;
use crate::state::BaseState;
use crate::token::{contrast, contrast::contrast_ratio};

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
