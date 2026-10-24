//! Tests for the `NumberBox` resolver and geometry — `UIF-ATOM-005`, §3.5.
//!
//! Five groups, in order:
//!
//! 1. **Delegation.** The entry half is §3.2's answer, not a second
//!    table that could disagree with the first.
//! 2. **The stepper matrix.** Background, perimeter and glyph per base
//!    state.
//! 3. **Contrast.** Every pair recomputed from raw sRGB, including the
//!    disabled glyph's placement inside the disabled-text band.
//! 4. **Geometry.** The two stepper shapes, their hit targets, and which
//!    direction a point fell in.
//! 5. **Layers and chrome classification.**

use super::*;
use crate::state::BaseState;
use crate::token::color::Palette;
use crate::token::contrast;
use crate::widget::text_input::{self, InputKind};
use iced::{Point, Rectangle};

/// The snapshot world every ratio in this file is measured against.
fn palette() -> Palette {
    Palette::dark()
}

/// A `VisualState` with no focus ring.
fn state(base: BaseState) -> VisualState {
    VisualState {
        base,
        focused: false,
    }
}

/// A `VisualState` carrying the keyboard ring.
fn focused(base: BaseState) -> VisualState {
    VisualState {
        base,
        focused: true,
    }
}

/// Assert a measured ratio against its true value to four places.
fn assert_ratio(actual: f32, expected: f32, what: &str) {
    let measured = f64::from(actual);
    let wanted = f64::from(expected);
    assert!(
        (measured - wanted).abs() < 0.000_5,
        "{what}: expected {wanted:.4}, measured {measured:.4}"
    );
}

/// A 180 × 32 number box.
fn bounds() -> Rectangle {
    Rectangle::new(Point::new(20.0, 50.0), iced::Size::new(180.0, ENTRY_HEIGHT))
}

// ---------------------------------------------------------------------------
// 1. Delegation — the entry is §3.2, once
// ---------------------------------------------------------------------------

#[test]
fn the_entry_half_is_exactly_what_the_text_entry_resolver_returns() {
    // Not "similar to". Identical. A second table describing what a
    // numeric field looks like is a second table that can drift.
    let p = palette();
    for base in [
        BaseState::Rest,
        BaseState::Hovered,
        BaseState::Pressed,
        BaseState::Disabled,
        BaseState::Invalid,
    ] {
        for ring in [false, true] {
            let v = VisualState {
                base,
                focused: ring,
            };
            let box_style = resolve_style(v, &p);
            let direct = text_input::resolve_input_style(InputKind::TextEntry, v, &p);
            assert_eq!(box_style.entry, direct, "{v:?} entry must be §3.2 verbatim");
        }
    }
}

#[test]
fn the_entry_takes_3_2_s_focus_indication_and_no_more() {
    // §3.2 indicates focus on a valid entry with its own border — a 2px
    // `accent.border` — and reserves the outside ring for the one case
    // where the border is already spoken for: invalid *and* focused.
    // Delegating means the number box inherits that split exactly, so
    // asserting a ring here would be asserting a second focus indicator
    // §3.2 does not have.
    let p = palette();
    let plain = resolve_style(state(BaseState::Rest), &p);
    let ringed = resolve_style(focused(BaseState::Rest), &p);
    assert_eq!(plain.entry.focus_ring_width, 0.0, "no outside ring at rest");
    assert_eq!(
        ringed.entry.focus_ring_width, 0.0,
        "and none when focused, because §3.2 puts focus on the border"
    );
    assert_eq!(
        ringed.entry.border_color, p.accent_border,
        "the focus indication is the border"
    );
    assert_eq!(
        ringed.entry.border_width,
        crate::token::shape::BORDER_FOCUS,
        "at the attention weight"
    );
    assert_ne!(
        ringed.entry.border_color, plain.entry.border_color,
        "and it is provably not the resting border"
    );

    let invalid_focused = resolve_style(focused(BaseState::Invalid), &p);
    assert!(
        invalid_focused.entry.focus_ring_width > 0.0,
        "the outside ring appears exactly where §3.2 needs it"
    );
}

#[test]
fn a_disabled_number_box_disables_its_entry_through_3_2_too() {
    let p = palette();
    let off = resolve_style(state(BaseState::Disabled), &p);
    let on = resolve_style(state(BaseState::Rest), &p);
    assert_ne!(off.entry.background, on.entry.background, "the field dims");
    assert_ne!(off.entry.text, on.entry.text, "and its digits dim with it");
}

// ---------------------------------------------------------------------------
// 2. The stepper matrix
// ---------------------------------------------------------------------------

#[test]
fn the_resting_stepper_is_raised_with_a_divider_and_a_muted_glyph() {
    let p = palette();
    let s = resolve_style(state(BaseState::Rest), &p);
    assert_eq!(s.stepper_background, p.surface_raised, "resting surface");
    assert_eq!(s.stepper_border, p.border_subtle, "a resting divider");
    assert_eq!(
        s.stepper_glyph, p.text_muted,
        "an actionable but untouched glyph"
    );
}

#[test]
fn the_hovered_stepper_brightens_its_surface_and_sharpens_its_glyph() {
    let p = palette();
    let s = resolve_style(state(BaseState::Hovered), &p);
    assert_eq!(s.stepper_background, p.surface_hover, "hover surface");
    assert_eq!(s.stepper_border, p.border_strong, "active chrome");
    assert_eq!(s.stepper_glyph, p.text_strong, "the glyph comes forward");
}

#[test]
fn the_pressed_stepper_goes_one_step_darker_than_the_hover() {
    let p = palette();
    let hover = resolve_style(state(BaseState::Hovered), &p);
    let press = resolve_style(state(BaseState::Pressed), &p);
    assert_eq!(press.stepper_background, p.surface_active, "press surface");
    assert_ne!(
        press.stepper_background, hover.stepper_background,
        "a press must be distinguishable from a hover"
    );
    assert_eq!(press.stepper_border, p.border_strong);
    assert_eq!(press.stepper_glyph, p.text_strong);
}

#[test]
fn the_disabled_stepper_keeps_its_surface_and_only_dims_the_glyph() {
    // A disabled stepper still has to be *seen* — it is the reason the
    // value cannot be changed. Only the glyph gives up its liveness.
    let p = palette();
    let off = resolve_style(state(BaseState::Disabled), &p);
    let rest = resolve_style(state(BaseState::Rest), &p);
    assert_eq!(
        off.stepper_background, p.surface_raised,
        "same surface as rest"
    );
    assert_eq!(off.stepper_border, p.border_subtle, "same divider as rest");
    assert_eq!(off.stepper_glyph, p.text_disabled, "a dead glyph");
    assert_ne!(
        off.stepper_glyph, rest.stepper_glyph,
        "and provably not the live one"
    );
}

#[test]
fn invalid_leaves_the_stepper_alone_and_reaches_the_entry_through_3_2() {
    // §3B.1 renders a field's validation words in the reserved caption
    // slot rather than on the control, and §3.5 gives the stepper no
    // invalid cell of its own — so the stepper keeps its resting skin
    // outright. The entry is the other half of the story: it *is*
    // §3.2's, and §3.2 does paint an invalid input. Delegation means the
    // number box inherits that treatment without holding a second copy of
    // the table that could drift from the first.
    let p = palette();
    let invalid = resolve_style(state(BaseState::Invalid), &p);
    let rest = resolve_style(state(BaseState::Rest), &p);
    assert_eq!(
        invalid.stepper_background, rest.stepper_background,
        "the stepper does not know about validation"
    );
    assert_eq!(invalid.stepper_border, rest.stepper_border);
    assert_eq!(invalid.stepper_glyph, rest.stepper_glyph);
    assert_ne!(
        invalid.entry, rest.entry,
        "the entry carries §3.2's invalid skin"
    );
    assert_eq!(
        invalid.entry,
        text_input::resolve_input_style(InputKind::TextEntry, state(BaseState::Invalid), &p),
        "and it is §3.2's answer, not one invented here"
    );
}

#[test]
fn resolving_is_deterministic() {
    let p = palette();
    for base in [
        BaseState::Rest,
        BaseState::Hovered,
        BaseState::Pressed,
        BaseState::Disabled,
        BaseState::Invalid,
    ] {
        for ring in [false, true] {
            let v = VisualState {
                base,
                focused: ring,
            };
            assert_eq!(
                resolve_style(v, &p),
                resolve_style(v, &p),
                "{v:?} must resolve the same way twice"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3. Contrast — recomputed from raw sRGB
// ---------------------------------------------------------------------------

#[test]
fn the_resting_glyph_clears_text_contrast_on_its_own_background() {
    let p = palette();
    let s = resolve_style(state(BaseState::Rest), &p);
    assert_ratio(
        contrast::contrast_ratio(s.stepper_glyph, s.stepper_background),
        7.1553,
        "text.muted / surface.raised",
    );
    assert!(contrast::meets_text_contrast(
        s.stepper_glyph,
        s.stepper_background
    ));
}

#[test]
fn the_hovered_and_pressed_glyphs_clear_text_contrast() {
    let p = palette();
    for (base, bg, want) in [
        (BaseState::Hovered, palette().surface_hover, 16.2351),
        (BaseState::Pressed, palette().surface_active, 14.2946),
    ] {
        let s = resolve_style(state(base), &p);
        assert_eq!(s.stepper_background, bg, "{base:?} surface");
        assert_ratio(
            contrast::contrast_ratio(s.stepper_glyph, s.stepper_background),
            want,
            &format!("{base:?} glyph on its surface"),
        );
        assert!(contrast::meets_text_contrast(
            s.stepper_glyph,
            s.stepper_background
        ));
    }
}

#[test]
fn the_disabled_glyph_sits_inside_the_disabled_text_band() {
    // Not merely "dim enough" — the band has a ceiling as well as a floor,
    // so a disabled control reads as inert rather than as a low-contrast
    // live one.
    let p = palette();
    let s = resolve_style(state(BaseState::Disabled), &p);
    let ratio = contrast::contrast_ratio(s.stepper_glyph, s.stepper_background);
    assert_ratio(ratio, 5.5643, "text.disabled / surface.raised");
    assert!(
        contrast::meets_disabled_text_band(s.stepper_glyph, s.stepper_background),
        "disabled glyph must land in the 4.0–6.0 band, got {ratio:.4}"
    );
}

#[test]
fn the_active_stepper_perimeter_clears_the_chrome_floor() {
    let p = palette();
    for (base, bg, want) in [
        (BaseState::Hovered, palette().surface_hover, 3.6772),
        (BaseState::Pressed, palette().surface_active, 3.2377),
    ] {
        let s = resolve_style(state(base), &p);
        assert_eq!(s.stepper_border, p.border_strong, "{base:?} perimeter");
        assert_ratio(
            contrast::contrast_ratio(s.stepper_border, bg),
            want,
            &format!("{base:?} perimeter against its own surface"),
        );
        assert!(contrast::meets_chrome_contrast(s.stepper_border, bg));
    }
}

#[test]
fn the_active_perimeter_clears_every_mount_the_control_can_sit_on() {
    let p = palette();
    let mounts = [
        ("base", p.surface_base),
        ("raised", p.surface_raised),
        ("overlay", p.surface_overlay),
        ("hover", p.surface_hover),
        ("active", p.surface_active),
    ];
    for base in [BaseState::Hovered, BaseState::Pressed] {
        let s = resolve_style(state(base), &p);
        for (name, mount) in mounts {
            assert!(
                contrast::meets_chrome_contrast(s.stepper_border, mount),
                "{base:?} stepper perimeter on {name}: {:.4}",
                contrast::contrast_ratio(s.stepper_border, mount)
            );
        }
    }
}

#[test]
fn the_glyph_is_legible_against_every_surface_the_stepper_block_can_take() {
    let p = palette();
    let surfaces = [
        ("raised", p.surface_raised),
        ("hover", p.surface_hover),
        ("active", p.surface_active),
    ];
    for base in [BaseState::Rest, BaseState::Hovered, BaseState::Pressed] {
        let s = resolve_style(state(base), &p);
        for (name, bg) in surfaces {
            assert!(
                contrast::meets_text_contrast(s.stepper_glyph, bg),
                "{base:?} glyph against {name}: {:.4}",
                contrast::contrast_ratio(s.stepper_glyph, bg)
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Geometry
// ---------------------------------------------------------------------------

#[test]
fn the_control_is_32_tall_at_the_forms_rung() {
    assert_eq!(ENTRY_HEIGHT, 32.0, "§3.5's 32px numeric entry");
    let rect = control_rect(Point::new(5.0, 7.0), 200.0);
    assert_eq!(rect.height, ENTRY_HEIGHT);
    assert_eq!(rect.width, 200.0);
    assert_eq!(rect.x, 5.0);
    assert_eq!(rect.y, 7.0);
}

#[test]
fn the_stepper_is_the_trailing_column_and_the_entry_takes_the_rest() {
    let b = bounds();
    let stepper = stepper_rect(b);
    let entry = entry_rect(b);
    assert_eq!(stepper.width, STEPPER_WIDTH, "28px column");
    assert_eq!(
        stepper.x + stepper.width,
        b.x + b.width,
        "flush to the trailing edge"
    );
    assert_eq!(
        entry.width + stepper.width,
        b.width,
        "the two halves tile the control with no overlap and no gap"
    );
    assert_eq!(entry.x, b.x, "the entry starts where the control does");
}

#[test]
fn a_control_narrower_than_the_stepper_does_not_underflow_the_entry() {
    let narrow = Rectangle::new(Point::ORIGIN, iced::Size::new(10.0, ENTRY_HEIGHT));
    let entry = entry_rect(narrow);
    assert!(entry.width >= 0.0, "no negative width");
    assert_eq!(
        entry.width + stepper_rect(narrow).width,
        narrow.width,
        "and the halves still tile"
    );
}

#[test]
fn the_stacked_pair_splits_the_block_height_in_two_sixteens() {
    let b = bounds();
    let [dec, inc] = step_button_rects(b, StepperShape::Stacked);
    assert_eq!(dec.width, STEPPER_WIDTH, "full column width");
    assert_eq!(dec.height, 16.0, "§3.5's 28 × 16 compact buttons");
    assert_eq!(inc.height, 16.0);
    assert_eq!(dec.y + dec.height, inc.y, "stacked with no gap");
    assert_eq!(dec.y, b.y, "top button at the block top");
    assert_eq!(
        inc.y + inc.height,
        b.y + b.height,
        "bottom button at the block bottom"
    );
}

#[test]
fn the_side_by_side_pair_uses_24_square_buttons_abreast() {
    let b = bounds();
    let [dec, inc] = step_button_rects(b, StepperShape::SideBySide);
    assert_eq!(dec.width, STEP_BUTTON_SIZE, "24px");
    assert_eq!(dec.height, STEP_BUTTON_SIZE, "24px square");
    assert_eq!(inc.width, STEP_BUTTON_SIZE);
    assert_eq!(inc.height, STEP_BUTTON_SIZE);
    assert_eq!(dec.y, inc.y, "abreast, so level");
    assert!(
        dec.x < inc.x,
        "decrement reads first on the left: {} vs {}",
        dec.x,
        inc.x
    );
    assert_eq!(dec.x + dec.width, inc.x, "touching, with no gap between");
    assert_eq!(
        inc.x + inc.width,
        b.x + b.width,
        "and the pair runs out to the trailing edge"
    );
}

#[test]
fn the_side_by_side_zone_is_the_full_control_height_with_the_buttons_centred() {
    // §3.5 names the two trailing zones `28 x 32` and `48 x 32`. Both
    // are as tall as the field beside them, so the 48px zone is not a
    // 24px-tall wrapper that shrinks to hold its buttons. The 8px the
    // zone is taller than a button is split 4px above and 4px below.
    let b = bounds();
    let zone = stepper_rect_for(StepperShape::SideBySide, b);
    assert_eq!(zone.width, 48.0, "two 24px squares of zone");
    assert_eq!(zone.height, ENTRY_HEIGHT, "the full 32px, not 24px");
    assert_eq!(zone.y, b.y, "and it starts at the control's top");
    let [dec, inc] = step_button_rects(b, StepperShape::SideBySide);
    let slack = (zone.height - dec.height) / 2.0;
    assert_eq!(slack, 4.0, "32 minus 24, halved");
    assert_eq!(dec.y - zone.y, slack, "4px of zone above the buttons");
    assert_eq!(
        zone.y + zone.height - (inc.y + inc.height),
        slack,
        "and the same 4px below"
    );
    assert!(
        dec.y > zone.y,
        "the buttons do not sit on the zone's top edge"
    );
    assert!(
        inc.y + inc.height < zone.y + zone.height,
        "nor on its bottom edge"
    );
    assert_eq!(
        zone.width,
        dec.width + inc.width,
        "the two squares fill the zone's width exactly"
    );
}

#[test]
fn the_two_step_buttons_never_overlap_in_either_shape() {
    // The bug the shape-aware block width exists to prevent. With two
    // `24px` squares forced into a `28px` column the rectangles overlapped
    // for `20px` of their height, and `step_at` — which tests decrement
    // first — returned `Decrement` at the increment button's own centre.
    // A `+` click that stepped the value down. Pinned for both shapes and
    // for both buttons' centres and corners.
    for shape in [StepperShape::Stacked, StepperShape::SideBySide] {
        let b = bounds();
        let [dec, inc] = step_button_rects(b, shape);
        assert!(
            dec.x + dec.width <= inc.x
                || inc.x + inc.width <= dec.x
                || dec.y + dec.height <= inc.y
                || inc.y + inc.height <= dec.y,
            "{shape:?}: the two buttons overlap"
        );
        assert_eq!(
            step_at(b, shape, centre(inc)),
            Some(StepDirection::Increment),
            "{shape:?}: the increment button's centre must read as increment"
        );
        assert_eq!(
            step_at(b, shape, centre(dec)),
            Some(StepDirection::Decrement),
            "{shape:?}: the decrement button's centre must read as decrement"
        );
    }
}

#[test]
fn the_side_by_side_block_is_wide_enough_for_what_it_holds() {
    // The spec gap, recorded as arithmetic: `28px` cannot hold two
    // `24px` squares, so the block is `48px` for this shape and `28px`
    // for the stacked one.
    let p = bounds();
    assert_eq!(stepper_width(StepperShape::Stacked), STEPPER_WIDTH);
    assert_eq!(
        stepper_width(StepperShape::SideBySide),
        2.0 * STEP_BUTTON_SIZE,
        "two 24px squares, and no less"
    );
    assert!(
        stepper_width(StepperShape::SideBySide) > STEPPER_WIDTH,
        "which is more than §3.5's block offers"
    );
    assert_eq!(
        stepper_rect_for(StepperShape::SideBySide, p).width,
        stepper_width(StepperShape::SideBySide)
    );
    assert_eq!(
        stepper_rect_for(StepperShape::Stacked, p),
        stepper_rect(p),
        "the shape-free accessor is the spec-literal stacked block"
    );
    assert_eq!(
        entry_rect_for(StepperShape::Stacked, p),
        entry_rect(p),
        "and so is the entry's"
    );
}

#[test]
fn a_narrow_box_shrinks_the_side_by_side_pair_rather_than_overlapping_it() {
    // The floor the width clamp protects: at 40px total the entry gets
    // 0px and the two buttons shrink to 20px each rather than overlapping
    // or running past the control.
    let narrow = Rectangle::new(Point::new(0.0, 0.0), iced::Size::new(40.0, ENTRY_HEIGHT));
    let [dec, inc] = step_button_rects(narrow, StepperShape::SideBySide);
    assert_eq!(dec.width, 20.0, "clamped to half the block");
    assert_eq!(dec.width, inc.width);
    assert_eq!(dec.x + dec.width, inc.x, "still no overlap");
    assert!(
        inc.x + inc.width <= narrow.x + narrow.width + 0.001,
        "and still inside the control"
    );
    assert_eq!(
        step_at(narrow, StepperShape::SideBySide, centre(inc)),
        Some(StepDirection::Increment),
        "and the click still means what the button says"
    );
}

#[test]
fn the_stacked_halves_are_below_the_minimum_target_and_the_side_by_side_pair_is_at_it() {
    // §3.5 prints both forms without comment on this point, so the
    // framework prints both back rather than quietly promoting the
    // compact variant. The gap is visible in the gallery cell.
    let [dec, _] = step_button_rects(bounds(), StepperShape::Stacked);
    assert!(
        dec.height < size::HIT_TARGET_MIN,
        "the 16px stacked half is under the 24px minimum: {}",
        dec.height
    );
    let [dec, _] = step_button_rects(bounds(), StepperShape::SideBySide);
    assert_eq!(
        dec.height,
        size::HIT_TARGET_MIN,
        "the side-by-side pair sits exactly on the floor"
    );
}

#[test]
fn step_at_reads_the_stacked_pair_top_and_bottom() {
    let b = bounds();
    let [dec, inc] = step_button_rects(b, StepperShape::Stacked);
    assert_eq!(
        step_at(b, StepperShape::Stacked, centre(dec)),
        Some(StepDirection::Decrement),
        "the upper half is ▼"
    );
    assert_eq!(
        step_at(b, StepperShape::Stacked, centre(inc)),
        Some(StepDirection::Increment),
        "the lower half is ▲"
    );
}

#[test]
fn step_at_reads_the_side_by_side_pair() {
    let b = bounds();
    let [dec, inc] = step_button_rects(b, StepperShape::SideBySide);
    assert_eq!(
        step_at(b, StepperShape::SideBySide, centre(dec)),
        Some(StepDirection::Decrement)
    );
    assert_eq!(
        step_at(b, StepperShape::SideBySide, centre(inc)),
        Some(StepDirection::Increment)
    );
}

#[test]
fn a_point_in_the_entry_is_not_a_step() {
    let b = bounds();
    assert_eq!(
        step_at(b, StepperShape::Stacked, Point::new(b.x + 10.0, b.y + 16.0)),
        None,
        "typing is not stepping"
    );
    assert_eq!(
        step_at(
            b,
            StepperShape::SideBySide,
            Point::new(b.x + 10.0, b.y + 16.0)
        ),
        None
    );
}

#[test]
fn a_point_outside_the_control_is_not_a_step() {
    let b = bounds();
    for point in [
        Point::new(b.x - 5.0, b.y + 16.0),
        Point::new(b.x + b.width + 5.0, b.y + 16.0),
        Point::new(b.x + b.width - 14.0, b.y - 5.0),
        Point::new(b.x + b.width - 14.0, b.y + b.height + 5.0),
    ] {
        assert_eq!(step_at(b, StepperShape::Stacked, point), None, "{point:?}");
        assert_eq!(
            step_at(b, StepperShape::SideBySide, point),
            None,
            "{point:?}"
        );
    }
}

#[test]
fn the_two_shapes_never_overlap_their_buttons_with_the_entry() {
    for shape in [StepperShape::Stacked, StepperShape::SideBySide] {
        let b = bounds();
        let entry = entry_rect_for(shape, b);
        for rect in step_button_rects(b, shape) {
            assert!(
                rect.x >= entry.x + entry.width - 1e-4,
                "{shape:?} buttons stay right of the entry: {} vs {}",
                rect.x,
                entry.x + entry.width
            );
        }
    }
}

#[test]
fn the_side_by_side_shape_takes_more_entry_width_than_the_stacked_one() {
    // The cost of the wider block, printed rather than hidden: the field
    // loses 20px of digits to the bigger buttons.
    let b = bounds();
    let stacked = entry_rect_for(StepperShape::Stacked, b);
    let abreast = entry_rect_for(StepperShape::SideBySide, b);
    assert!(
        abreast.width < stacked.width,
        "side-by-side is the wider block"
    );
    assert_eq!(
        stacked.width - abreast.width,
        stepper_width(StepperShape::SideBySide) - stepper_width(StepperShape::Stacked),
        "exactly the extra block width, nothing else"
    );
    assert_eq!(abreast.height, b.height, "the entry keeps the full height");
}

#[test]
fn every_direction_carries_a_distinct_glyph_per_shape() {
    let glyphs = [
        StepDirection::Decrement.glyph(StepperShape::Stacked),
        StepDirection::Increment.glyph(StepperShape::Stacked),
        StepDirection::Decrement.glyph(StepperShape::SideBySide),
        StepDirection::Increment.glyph(StepperShape::SideBySide),
    ];
    let distinct: std::collections::HashSet<char> = glyphs.iter().copied().collect();
    assert_eq!(
        distinct.len(),
        4,
        "four direction × shape pairs must not read the same: {glyphs:?}"
    );
    assert_eq!(
        StepDirection::Decrement.glyph(StepperShape::Stacked),
        GLYPH_DOWN
    );
    assert_eq!(
        StepDirection::Increment.glyph(StepperShape::Stacked),
        GLYPH_UP
    );
    assert_eq!(
        StepDirection::Decrement.glyph(StepperShape::SideBySide),
        GLYPH_MINUS
    );
    assert_eq!(
        StepDirection::Increment.glyph(StepperShape::SideBySide),
        GLYPH_PLUS
    );
}

#[test]
fn the_minus_glyph_is_a_minus_sign_not_a_hyphen() {
    assert_eq!(GLYPH_MINUS as u32, 0x2212, "U+2212 MINUS SIGN");
    assert_ne!(GLYPH_MINUS, '-');
}

#[test]
fn direction_delta_is_the_signed_step_count() {
    assert_eq!(StepDirection::Decrement.delta(), -1);
    assert_eq!(StepDirection::Increment.delta(), 1);
}

// ---------------------------------------------------------------------------
// 5. Layers and chrome classification
// ---------------------------------------------------------------------------

#[test]
fn the_stepper_layer_covers_the_trailing_column_with_a_hairline() {
    let p = palette();
    let s = resolve_style(state(BaseState::Rest), &p);
    let layer = stepper_layer(bounds(), &s);
    assert_eq!(layer.bounds, stepper_rect(bounds()));
    assert_eq!(layer.fill, p.surface_raised);
    assert_eq!(layer.border.color, p.border_subtle);
    assert_eq!(layer.border.width, BORDER_WIDTH);
    assert_eq!(layer.border.radius, RADIUS.into());
}

#[test]
fn the_step_button_layers_are_recorded_separately_in_decrement_first_order() {
    // So a snapshot can show which half a press fell in, the same reason
    // the split button's two zones are cells of their own.
    let p = palette();
    let s = resolve_style(state(BaseState::Hovered), &p);
    let layers = step_button_layers(bounds(), &s, StepperShape::Stacked);
    assert_eq!(
        layers[0].bounds,
        step_button_rects(bounds(), StepperShape::Stacked)[0]
    );
    assert_eq!(
        layers[1].bounds,
        step_button_rects(bounds(), StepperShape::Stacked)[1]
    );
    assert!(layers[0].bounds.y < layers[1].bounds.y, "decrement first");
    assert_eq!(layers[0].fill, p.surface_hover);
}

#[test]
fn the_entry_layer_is_the_delegated_background_at_the_entry_rect() {
    let p = palette();
    let s = resolve_style(state(BaseState::Rest), &p);
    let layer = entry_layer(bounds(), &s).expect("a resting entry paints");
    assert_eq!(
        layer.bounds,
        entry_rect(bounds()),
        "the entry, not the whole control"
    );
    assert_eq!(layer.fill, s.entry.background);
}

#[test]
fn the_entry_focus_ring_layer_is_delegated_and_appears_only_where_3_2_puts_one() {
    // Rest and plain-focus paint no outside ring layer, because §3.2
    // indicates those with the border. Invalid-and-focused is the one
    // cell that gets the outside ring, and the layer follows the style
    // rather than deciding anything.
    let p = palette();
    let plain = resolve_style(state(BaseState::Rest), &p);
    assert!(entry_focus_ring_layer(bounds(), &plain).is_none());
    let focused_valid = resolve_style(focused(BaseState::Rest), &p);
    assert!(
        entry_focus_ring_layer(bounds(), &focused_valid).is_none(),
        "focus alone is a border, not a ring"
    );
    let invalid_focused = resolve_style(focused(BaseState::Invalid), &p);
    let ring =
        entry_focus_ring_layer(bounds(), &invalid_focused).expect("invalid+focused needs a ring");
    assert_eq!(
        ring.bounds,
        entry_rect(bounds()).expand(invalid_focused.entry.focus_ring_width),
        "the ring sits outside the entry's own perimeter, §3.2-style"
    );
    assert_eq!(ring.border.width, invalid_focused.entry.focus_ring_width);
}

#[test]
fn the_resting_stepper_border_is_a_resting_divider_and_the_active_one_is_judged() {
    let p = palette();
    assert_eq!(
        stepper_border_chrome_class(p.border_subtle, &p),
        ChromeClass::RestingDivider,
        "the resting perimeter is passive structure"
    );
    assert_eq!(
        stepper_border_chrome_class(p.border_strong, &p),
        ChromeClass::Judged,
        "the hover and press perimeters are active chrome"
    );
    assert!(stepper_border_chrome_class(p.border_subtle, &p).is_exempt());
    assert!(!stepper_border_chrome_class(p.border_strong, &p).is_exempt());
}

#[test]
fn every_state_classifies_its_own_perimeter_consistently() {
    let p = palette();
    for base in [BaseState::Rest, BaseState::Invalid, BaseState::Disabled] {
        let s = resolve_style(state(base), &p);
        assert_eq!(
            stepper_border_chrome_class(s.stepper_border, &p),
            ChromeClass::RestingDivider,
            "{base:?} rests on a divider"
        );
    }
    for base in [BaseState::Hovered, BaseState::Pressed] {
        let s = resolve_style(state(base), &p);
        assert_eq!(
            stepper_border_chrome_class(s.stepper_border, &p),
            ChromeClass::Judged,
            "{base:?} is active chrome"
        );
    }
}

/// The centre of a rectangle, for hit-test probes.
fn centre(rect: Rectangle) -> Point {
    Point::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
}
