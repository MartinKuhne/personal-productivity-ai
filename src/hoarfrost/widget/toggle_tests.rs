//! Tests for the toggle-family resolver and geometry — `UIF-ATOM-003`.
//!
//! Four groups, in order:
//!
//! 1. **The matrix.** Every cell of §3.3's two rows transcribed as an
//!    expected `(fill, border, mark, label)` quad and asserted against
//!    `resolve_style`, so the resolver is checked against the spec
//!    rather than against itself.
//! 2. **Focus, the fill lock and the stroke class.** The rules that are
//!    not plain table lookups: the ring is additive and *computed* from
//!    the fill it lands on, the checked fills carry the `UIF-TOK-006`
//!    rule 4 lock, and the perimeter stroke classifies rather than being
//!    held to one threshold.
//! 3. **The recomputed contrast table.** Every ratio §3.3 prints,
//!    re-derived from the raw sRGB in `Palette::dark` through the
//!    crate's own `contrast_ratio` — including the two printed figures
//!    that do not survive.
//! 4. **Geometry.** The §3.3 geometry paragraph and the §2.2 rungs,
//!    asserted to be the tokens they are sourced from rather than
//!    restated numbers.
//!
//! # Why the ratios are recomputed rather than transcribed
//!
//! The spec's `Ratio` column is a claim about arithmetic, and arithmetic
//! can be checked. Every figure asserted here comes out of
//! `contrast_ratio` on the palette's own channels, so a palette retune
//! moves these numbers and a test failure names the pair that moved. Two
//! of the spec's printed figures turn out to be wrong by more than
//! rounding — see
//! [`two_of_the_specs_printed_ratios_do_not_survive_recomputation`].

use super::*;
use crate::state::{BaseState, InteractionInput, VisualState, resolve};
use crate::token::color::Palette;
use crate::token::contrast::{self, ChromeClass, FillLock};
use crate::token::{shape, size, space};
use crate::widget::focus;
use iced::Color;

/// Both polarities, in spec row order.
const POLARITIES: [TogglePolarity; 2] = [TogglePolarity::Off, TogglePolarity::On];

/// Every base state the resolver is asked about.
const BASES: [BaseState; 5] = [
    BaseState::Rest,
    BaseState::Hovered,
    BaseState::Pressed,
    BaseState::Invalid,
    BaseState::Disabled,
];

/// Every control, in §3.3 order.
const CONTROLS: [ToggleControl; 3] = [
    ToggleControl::Checkbox,
    ToggleControl::Radio,
    ToggleControl::Switch,
];

/// Tolerance for a ratio asserted to four decimal places.
const RATIO_TOLERANCE: f32 = 1e-3;

fn palette() -> Palette {
    Palette::dark()
}

fn style(polarity: TogglePolarity, base: BaseState, focused: bool) -> ToggleStyle {
    resolve_style(polarity, VisualState { base, focused }, &palette())
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < RATIO_TOLERANCE
}

// ---------------------------------------------------------------------------
// 1. The matrix
// ---------------------------------------------------------------------------

#[test]
fn off_row_matches_the_spec_matrix() {
    let p = palette();

    let rest = style(TogglePolarity::Off, BaseState::Rest, false);
    assert_eq!(rest.indicator_fill, p.surface_code);
    assert_eq!(rest.indicator_border, p.border_strong);
    assert_eq!(rest.mark, p.text_muted);
    assert_eq!(rest.label, p.text_primary);

    let hovered = style(TogglePolarity::Off, BaseState::Hovered, false);
    assert_eq!(hovered.indicator_fill, p.surface_hover);
    assert_eq!(hovered.indicator_border, p.accent_border);
    assert_eq!(hovered.mark, p.text_strong);
    assert_eq!(hovered.label, p.text_strong);

    let pressed = style(TogglePolarity::Off, BaseState::Pressed, false);
    assert_eq!(pressed.indicator_fill, p.surface_active);
    assert_eq!(pressed.indicator_border, p.accent_border);
    assert_eq!(pressed.mark, p.text_strong);
    assert_eq!(pressed.label, p.text_strong);

    let disabled = style(TogglePolarity::Off, BaseState::Disabled, false);
    assert_eq!(disabled.indicator_fill, p.surface_raised);
    assert_eq!(disabled.indicator_border, p.border_subtle);
    assert_eq!(disabled.mark, p.text_disabled);
    assert_eq!(disabled.label, p.text_disabled);
}

#[test]
fn on_row_matches_the_spec_matrix() {
    let p = palette();

    let rest = style(TogglePolarity::On, BaseState::Rest, false);
    assert_eq!(rest.indicator_fill, p.accent_fill);
    assert_eq!(rest.indicator_border, p.accent_fill);
    assert_eq!(rest.mark, p.text_strong);
    assert_eq!(rest.label, p.text_primary);

    let hovered = style(TogglePolarity::On, BaseState::Hovered, false);
    assert_eq!(hovered.indicator_fill, p.accent_fill);
    assert_eq!(hovered.indicator_border, p.text_strong);
    assert_eq!(hovered.mark, p.text_strong);
    assert_eq!(hovered.label, p.text_strong);

    let pressed = style(TogglePolarity::On, BaseState::Pressed, false);
    assert_eq!(pressed.indicator_fill, p.accent_fill_active);
    assert_eq!(pressed.indicator_border, p.accent_fill_active);
    assert_eq!(pressed.mark, p.text_strong);
    assert_eq!(pressed.label, p.text_strong);

    let disabled = style(TogglePolarity::On, BaseState::Disabled, false);
    assert_eq!(disabled.indicator_fill, p.surface_raised);
    assert_eq!(disabled.indicator_border, p.border_subtle);
    assert_eq!(disabled.mark, p.text_disabled);
    assert_eq!(disabled.label, p.text_disabled);
}

#[test]
fn the_off_label_is_primary_at_rest_and_strong_once_touched() {
    // The `7.04:1` §3.3 prints beside `text.muted` is the switch THUMB on
    // `surface.code`, not the label. Reading it as the label's would have
    // painted a 7:1 grey where the spec asks for a 12:1 one.
    let p = palette();
    assert_eq!(
        style(TogglePolarity::Off, BaseState::Rest, false).label,
        p.text_primary
    );
    assert_eq!(
        style(TogglePolarity::Off, BaseState::Hovered, false).label,
        p.text_strong
    );
    assert_eq!(
        style(TogglePolarity::Off, BaseState::Pressed, false).label,
        p.text_strong
    );
    assert_ne!(
        style(TogglePolarity::Off, BaseState::Rest, false).label,
        p.text_muted,
        "the off-polarity label is `text.primary`; `text.muted` belongs to the thumb channel"
    );
}

#[test]
fn invalid_paints_exactly_as_rest_for_both_polarities() {
    // §3.3 gives a toggle no invalid skin, so the resolver paints
    // `Invalid` as `Rest` rather than inventing a cell. Pinned so a future
    // invalid-toggle requirement has to be added deliberately.
    for polarity in POLARITIES {
        assert_eq!(
            style(polarity, BaseState::Invalid, false),
            style(polarity, BaseState::Rest, false),
            "{polarity:?}: Invalid must track Rest while the spec gives no invalid cell"
        );
    }
}

#[test]
fn resolve_style_is_a_pure_function_of_its_inputs() {
    for polarity in POLARITIES {
        for base in BASES {
            for focused in [false, true] {
                let state = VisualState { base, focused };
                let first = resolve_style(polarity, state, &palette());
                let second = resolve_style(polarity, state, &palette());
                assert_eq!(
                    first, second,
                    "{polarity:?} {state:?}: resolve_style must not remember the previous call"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 2. Focus, the fill lock and the stroke class
// ---------------------------------------------------------------------------

#[test]
fn the_focus_ring_is_additive_and_never_replaces_the_base() {
    for polarity in POLARITIES {
        for base in BASES {
            let bare = style(polarity, base, false);
            // `Disabled + focused` is not a state the machine can produce —
            // `resolve` zeroes the ring on a dead control — so it is not
            // hand-constructed here. The dedicated test further down
            // drives that combination through `resolve` instead.
            let focused = base != BaseState::Disabled;
            let ringed = style(polarity, base, focused);

            assert_eq!(bare.indicator_fill, ringed.indicator_fill);
            assert_eq!(bare.indicator_border, ringed.indicator_border);
            assert_eq!(bare.mark, ringed.mark);
            assert_eq!(bare.label, ringed.label);

            if focused {
                assert_eq!(
                    ringed.focus_ring_width,
                    shape::BORDER_FOCUS,
                    "{polarity:?} {base:?}: focus must add the ring on top, not trade it \
                     for anything in the base"
                );
                assert_ne!(ringed.focus_ring_color.a, 0.0);
            } else {
                assert_eq!(ringed.focus_ring_width, 0.0);
            }
        }
    }
}

#[test]
fn an_unfocused_style_draws_no_ring_at_all() {
    for polarity in POLARITIES {
        for base in BASES {
            let s = style(polarity, base, false);
            assert_eq!(s.focus_ring_width, 0.0);
            assert_eq!(s.focus_ring_color, Color::TRANSPARENT);
        }
    }
}

#[test]
fn the_off_ring_is_the_accent_because_the_accent_clears_the_floor() {
    let p = palette();
    for base in [BaseState::Rest, BaseState::Hovered, BaseState::Pressed] {
        let s = style(TogglePolarity::Off, base, true);
        assert_eq!(
            s.focus_ring_color, p.accent_border,
            "{base:?}: the off fill lets the accent be seen, so the accent wins"
        );
        assert!(
            contrast::contrast_ratio(p.accent_border, s.indicator_fill)
                >= contrast::MIN_CHROME_CONTRAST,
            "{base:?}: the ring is only the accent if it actually clears 3.0:1"
        );
    }
}

#[test]
fn the_on_ring_is_white_because_the_accent_cannot_be_seen_on_the_fill() {
    let p = palette();
    for base in [BaseState::Rest, BaseState::Hovered, BaseState::Pressed] {
        let s = style(TogglePolarity::On, base, true);
        let accent_on_fill = contrast::contrast_ratio(p.accent_border, s.indicator_fill);
        assert!(
            accent_on_fill < contrast::MIN_CHROME_CONTRAST,
            "{base:?}: the premise — the accent is invisible on the accent fill — must hold, \
             measured {accent_on_fill}"
        );
        assert_eq!(
            s.focus_ring_color, p.text_strong,
            "{base:?}: the ring must escalate to `text.strong`, not stay invisible"
        );
    }
}

#[test]
fn the_ring_colour_is_never_hardcoded_per_polarity_but_matches_what_the_spec_prints() {
    // §3.3 prints `accent.border` for the Off row and `text.strong` for
    // the On row. Neither is written into the resolver: both fall out of
    // resolving the ring against the fill. This asserts the two printed
    // colours are reproduced, and that the shared rule is what produced
    // them.
    let p = palette();
    assert_eq!(
        style(TogglePolarity::Off, BaseState::Rest, true).focus_ring_color,
        focus::resolve_ring_color_for_background(p.surface_code, &p)
    );
    assert_eq!(
        style(TogglePolarity::On, BaseState::Rest, true).focus_ring_color,
        focus::resolve_ring_color_for_background(p.accent_fill, &p)
    );
}

#[test]
fn a_disabled_toggle_never_wears_a_ring_even_when_asked_for_one() {
    let state = resolve(InteractionInput {
        enabled: false,
        hovered: false,
        pressed: false,
        focused: true,
        invalid: false,
    });
    assert_eq!(state.base, BaseState::Disabled);
    assert!(!state.ring_visible());

    for polarity in POLARITIES {
        assert_eq!(
            style(polarity, state.base, state.focused).focus_ring_width,
            0.0
        );
    }
}

#[test]
fn the_checked_polarity_holds_the_rule_4_fill_lock() {
    let p = palette();
    for base in [
        BaseState::Rest,
        BaseState::Hovered,
        BaseState::Pressed,
        BaseState::Invalid,
    ] {
        let s = style(TogglePolarity::On, base, false);
        let lock = contrast::fill_lock(s.indicator_fill, s.mark, &p);
        assert_eq!(
            lock,
            FillLock::Locked,
            "{base:?}: a locked solid fill must carry `text.strong`, and the resolver must \
             have been through the assertion"
        );
        assert!(lock.is_compliant());
    }
}

#[test]
fn the_off_polarity_is_not_a_locked_fill_so_the_rule_never_applied() {
    let p = palette();
    for base in BASES {
        let s = style(TogglePolarity::Off, base, false);
        let lock = contrast::fill_lock(s.indicator_fill, s.mark, &p);
        assert_eq!(
            lock,
            FillLock::NotSolidFill,
            "{base:?}: the off fill is a surface, so rule 4 places no demand through it"
        );
    }
}

#[test]
fn the_chrome_class_matches_what_the_spec_prints() {
    let p = palette();
    let expected: [((TogglePolarity, BaseState), ChromeClass); 8] = [
        ((TogglePolarity::Off, BaseState::Rest), ChromeClass::Judged),
        (
            (TogglePolarity::Off, BaseState::Hovered),
            ChromeClass::Judged,
        ),
        (
            (TogglePolarity::Off, BaseState::Pressed),
            ChromeClass::Judged,
        ),
        (
            (TogglePolarity::Off, BaseState::Disabled),
            ChromeClass::RestingDivider,
        ),
        (
            (TogglePolarity::On, BaseState::Rest),
            ChromeClass::CoincidentWithFill,
        ),
        (
            (TogglePolarity::On, BaseState::Hovered),
            ChromeClass::Judged,
        ),
        (
            (TogglePolarity::On, BaseState::Pressed),
            ChromeClass::CoincidentWithFill,
        ),
        (
            (TogglePolarity::On, BaseState::Disabled),
            ChromeClass::RestingDivider,
        ),
    ];

    for ((polarity, base), class) in expected {
        let s = style(polarity, base, false);
        assert_eq!(
            indicator_chrome_class(s.indicator_border, s.indicator_fill, &p),
            class,
            "{polarity:?} {base:?}: the border class must be derived from token identity, \
             matching what §3.3 prints"
        );
    }
}

#[test]
fn every_judged_border_clears_the_chrome_floor() {
    let p = palette();
    for polarity in POLARITIES {
        for base in BASES {
            let s = style(polarity, base, false);
            let class = indicator_chrome_class(s.indicator_border, s.indicator_fill, &p);
            if class == ChromeClass::Judged {
                let ratio = contrast::contrast_ratio(s.indicator_border, s.indicator_fill);
                assert!(
                    ratio >= contrast::MIN_CHROME_CONTRAST,
                    "{polarity:?} {base:?}: judged border measured {ratio:.4}, below the \
                     {} chrome floor",
                    contrast::MIN_CHROME_CONTRAST
                );
            }
        }
    }
}

#[test]
fn no_toggle_cell_produces_the_unpainted_background_class() {
    // §3.3 never paints a transparent indicator, so a resolver that
    // could reach `UnpaintedBackground` would be modelling a state the
    // spec does not have.
    let p = palette();
    for polarity in POLARITIES {
        for base in BASES {
            let s = style(polarity, base, false);
            assert_ne!(s.indicator_fill, Color::TRANSPARENT);
            assert_ne!(s.indicator_border, Color::TRANSPARENT);
            assert_ne!(
                indicator_chrome_class(s.indicator_border, s.indicator_fill, &p),
                ChromeClass::UnpaintedBackground
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3. The recomputed contrast table
// ---------------------------------------------------------------------------

#[test]
fn every_ratio_the_spec_prints_that_is_correct_reproduces_exactly() {
    let p = palette();
    let agreeing: &[(&str, Color, Color, f32)] = &[
        (
            "border.strong / surface.code",
            p.border_strong,
            p.surface_code,
            4.1672,
        ),
        (
            "text.muted / surface.code",
            p.text_muted,
            p.surface_code,
            7.0358,
        ),
        (
            "text.strong / surface.hover",
            p.text_strong,
            p.surface_hover,
            16.2351,
        ),
        (
            "accent.border / surface.active",
            p.accent_border,
            p.surface_active,
            4.7922,
        ),
        (
            "text.strong / surface.active",
            p.text_strong,
            p.surface_active,
            14.2946,
        ),
        (
            "text.strong / accent.fill",
            p.text_strong,
            p.accent_fill,
            4.8020,
        ),
        (
            "text.strong / accent.fill_active",
            p.text_strong,
            p.accent_fill_active,
            5.6559,
        ),
    ];

    for (name, fg, bg, printed) in agreeing {
        let computed = contrast::contrast_ratio(*fg, *bg);
        assert!(
            close(computed, *printed),
            "{name}: spec prints {printed}, recomputed {computed:.4}"
        );
    }
}

#[test]
fn two_of_the_specs_printed_ratios_do_not_survive_recomputation() {
    let p = palette();

    // §3.3 prints `5.35:1` for the hovered off-polarity border.
    let hovered = contrast::contrast_ratio(p.accent_border, p.surface_hover);
    assert!(
        close(hovered, 5.4427),
        "true value is 5.4427, got {hovered:.4}"
    );
    assert!(
        (hovered - 5.35).abs() > 0.05,
        "the spec's 5.35 is not a rounding of the true 5.4427"
    );

    // §3.3 prints `6.67:1` as the top of the off-polarity focused-ring
    // range.
    let ring = contrast::contrast_ratio(p.accent_border, p.surface_code);
    assert!(close(ring, 6.1680), "true value is 6.1680, got {ring:.4}");
    assert!(
        (ring - 6.67).abs() > 0.4,
        "the spec's 6.67 is not a rounding of the true 6.1680"
    );

    // Neither correction breaks a floor: both clear the 3.0:1 chrome rule
    // as comfortably as the printed figures did, which is why the
    // resolver follows the tokens rather than the printed column.
    assert!(hovered >= contrast::MIN_CHROME_CONTRAST);
    assert!(ring >= contrast::MIN_CHROME_CONTRAST);
}

#[test]
fn the_ring_candidates_on_the_checked_fill_are_both_recorded() {
    let p = palette();
    // These two are what force the escalation, and they are the numbers the
    // module doc quotes.
    assert!(close(
        contrast::contrast_ratio(p.accent_border, p.accent_fill),
        1.6098
    ));
    assert!(close(
        contrast::contrast_ratio(p.accent_border, p.accent_fill_active),
        1.8961
    ));
    assert!(close(
        contrast::contrast_ratio(p.text_strong, p.accent_fill),
        4.8020
    ));
    assert!(close(
        contrast::contrast_ratio(p.text_strong, p.accent_fill_active),
        5.6559
    ));
}

#[test]
fn the_coincident_fill_separation_holds_on_container_surfaces() {
    let p = palette();
    // A coincident border draws no boundary, so the fill has to separate
    // the control from what it is mounted to. On every surface a toggle
    // is actually mounted on — a card, a dialog, a code well — both
    // checked fills clear the 3.00:1 that class owes.
    let containers = [
        ("surface.sunken", p.surface_sunken, 4.2159),
        ("surface.base", p.surface_base, 4.1432),
        ("surface.raised", p.surface_raised, 3.8965),
        ("surface.code", p.surface_code, 3.8315),
        ("surface.overlay", p.surface_overlay, 3.6895),
    ];
    for (name, surface, expected) in containers {
        assert!(
            close(contrast::contrast_ratio(p.accent_fill, surface), expected),
            "{name}: accent.fill separation expected {expected}"
        );
        assert!(
            contrast::meets_coincident_fill_contrast(p.accent_fill, surface),
            "{name}: accent.fill must clear the coincident-fill floor"
        );
        assert!(
            contrast::meets_coincident_fill_contrast(p.accent_fill_active, surface),
            "{name}: accent.fill_active must clear the coincident-fill floor"
        );
    }
}

#[test]
fn the_coincident_fill_floor_does_not_hold_on_a_pressed_container() {
    // The honest limit, and it is not this atom's: `surface.active` is a
    // *pressed control's* fill, not a container, and the accent fills do
    // not clear 3.00:1 against it. The Primary button carries exactly the
    // same limitation, so a checked toggle pressed on a pressed row would
    // be under the coincident floor. Recorded rather than waived — the
    // class is exempt from the gate, which is why nothing catches this at
    // build time.
    let p = palette();
    assert!(close(
        contrast::contrast_ratio(p.accent_fill, p.surface_active),
        2.9768
    ));
    assert!(close(
        contrast::contrast_ratio(p.accent_fill_active, p.surface_active),
        2.5274
    ));
    assert!(!contrast::meets_coincident_fill_contrast(
        p.accent_fill,
        p.surface_active
    ));
    assert!(!contrast::meets_coincident_fill_contrast(
        p.accent_fill_active,
        p.surface_active
    ));
}

// ---------------------------------------------------------------------------
// 4. Geometry
// ---------------------------------------------------------------------------

#[test]
fn the_indicator_box_is_the_token_it_names() {
    assert_eq!(
        indicator_size(ToggleControl::Checkbox),
        (size::CHECKBOX_BOX, size::CHECKBOX_BOX)
    );
    assert_eq!(
        indicator_size(ToggleControl::Radio),
        (size::CHECKBOX_BOX, size::CHECKBOX_BOX),
        "the radio is the same square drawn round, which is what lets one hit test cover both"
    );
    assert_eq!(
        indicator_size(ToggleControl::Switch),
        (size::SWITCH_TRACK_WIDTH, size::SWITCH_TRACK_HEIGHT)
    );
}

#[test]
fn the_radius_is_widget_for_the_box_and_pill_for_the_round_two() {
    assert_eq!(
        indicator_radius(ToggleControl::Checkbox),
        shape::RADIUS_WIDGET
    );
    assert_eq!(indicator_radius(ToggleControl::Radio), shape::RADIUS_PILL);
    assert_eq!(indicator_radius(ToggleControl::Switch), shape::RADIUS_PILL);
}

#[test]
fn the_stroke_and_gap_constants_are_token_sourced() {
    assert_eq!(BORDER_WIDTH, shape::BORDER_HAIRLINE);
    assert_eq!(FOCUS_RING_WIDTH, shape::BORDER_FOCUS);
    assert_eq!(LABEL_GAP, space::SM);
    // Compile-time rather than runtime: the comparison is between constants, so a
    // const assertion makes the guarantee hold even if this test never runs.
    const _: () = assert!(
        FOCUS_RING_WIDTH > BORDER_WIDTH,
        "a focused control must never read as one that merely has a heavier border"
    );
}

#[test]
fn the_hit_target_extends_across_the_label() {
    let (indicator_w, _) = indicator_size(ToggleControl::Checkbox);
    let (width, height) = hit_size(ToggleControl::Checkbox, 80.0);

    assert_eq!(width, indicator_w + LABEL_GAP + 80.0);
    assert_eq!(height, size::HIT_TARGET_MIN);
    assert!(
        width > size::HIT_TARGET_MIN,
        "a labelled row is wider than the floor, which is the intended direction"
    );
}

#[test]
fn the_hit_target_floors_both_dimensions_for_every_control() {
    for control in CONTROLS {
        for label_width in [0.0, 1.0, 8.0, 24.0, 200.0] {
            let (width, height) = hit_size(control, label_width);
            assert!(
                width >= size::HIT_TARGET_MIN,
                "{control:?} label {label_width}: width {width} below the floor"
            );
            assert!(
                height >= size::HIT_TARGET_MIN,
                "{control:?} label {label_width}: height {height} below the floor"
            );
        }
    }
}

#[test]
fn the_switch_track_is_below_the_floor_and_gets_grown() {
    // The track is 20 tall, so the height floor is doing real work here
    // rather than being satisfied by accident.
    let (indicator_w, indicator_h) = indicator_size(ToggleControl::Switch);
    assert_eq!(indicator_h, 20.0);
    assert!(indicator_h < size::HIT_TARGET_MIN);

    let (_, height) = hit_size(ToggleControl::Switch, 0.0);
    assert_eq!(height, size::HIT_TARGET_MIN);
    assert!(indicator_w >= size::HIT_TARGET_MIN);
}

#[test]
fn a_negative_label_width_collapses_to_the_smallest_legal_target() {
    // A failed upstream text measurement must not produce a negative box.
    let (width, height) = hit_size(ToggleControl::Checkbox, -50.0);
    let (bare_width, bare_height) = hit_size(ToggleControl::Checkbox, 0.0);
    assert_eq!(width, bare_width.max(size::HIT_TARGET_MIN));
    assert_eq!(height, bare_height);
    assert!(width > 0.0 && height > 0.0);
}

#[test]
fn the_thumb_travel_is_derived_from_the_three_track_tokens() {
    assert_eq!(
        thumb_travel(),
        size::SWITCH_TRACK_WIDTH - size::SWITCH_THUMB - 2.0 * size::SWITCH_THUMB_INSET
    );
    assert_eq!(thumb_travel(), 16.0);
    assert_eq!(size::SWITCH_THUMB_INSET, 3.0);

    // The thumb fits inside the track at both ends with the inset clear.
    let (track_w, track_h) = indicator_size(ToggleControl::Switch);
    assert!(thumb_offset(false) >= 0.0);
    assert!(thumb_offset(true) + size::SWITCH_THUMB + size::SWITCH_THUMB_INSET <= track_w);
    assert!(size::SWITCH_THUMB < track_h);
}

#[test]
fn the_thumb_offsets_are_the_two_ends_of_the_travel() {
    assert_eq!(thumb_offset(false), 0.0);
    assert_eq!(thumb_offset(true), thumb_travel());
    assert_ne!(thumb_offset(false), thumb_offset(true));
}

#[test]
fn every_control_has_a_distinct_indicator_shape() {
    // The three controls share a colour matrix, so if their geometry
    // collapsed too the atoms would be indistinguishable.
    let checkbox = indicator_size(ToggleControl::Checkbox);
    let radio = indicator_size(ToggleControl::Radio);
    let switch_track = indicator_size(ToggleControl::Switch);

    assert_eq!(checkbox, radio);
    assert_ne!(checkbox, switch_track);
    assert_ne!(
        indicator_radius(ToggleControl::Checkbox),
        indicator_radius(ToggleControl::Switch)
    );
}
