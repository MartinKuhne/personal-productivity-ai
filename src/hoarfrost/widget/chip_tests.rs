//! Tests for the chip resolver and geometry — `UIF-ATOM-006`, §3.6.
//!
//! Five groups, in order:
//!
//! 1. **The matrix.** Every cell of §3.6's two rows transcribed as an
//!    expected `(fill, border, text, glyph)` quad and asserted against
//!    `resolve_style`, plus the four cells §3.6 does not print and this
//!    module decides.
//! 2. **Focus, the fill lock and the perimeter class.** The rules that
//!    are not table lookups: the ring is additive and *computed* from
//!    the fill it lands on, the selected fill carries the `UIF-TOK-006`
//!    rule 4 lock, and the perimeter classifies by token identity.
//! 3. **The recomputed contrast table.** Every ratio §3.6 prints,
//!    re-derived from the raw sRGB in `Palette::dark` through the
//!    crate's own `contrast_ratio`, plus the mount that is forbidden.
//! 4. **Shape and slots.** The radius mapping, including the `Assist`
//!    gap §3.6 leaves open.
//! 5. **Geometry.** Width, the reserved `×` slot on both rungs, and the
//!    coupling between what `chip_width` charges and what `remove_slot`
//!    returns.

use super::*;
use crate::state::{BaseState, InteractionInput, VisualState, resolve};
use crate::token::color::Palette;
use crate::token::contrast::{self, ChromeClass, FillLock};
use crate::token::{shape, size, space};
use crate::widget::chip_model::ChipVariant;
use iced::{Color, Point, Rectangle, Size};

/// Both polarities, in spec row order.
const POLARITIES: [ChipPolarity; 2] = [ChipPolarity::Unselected, ChipPolarity::Selected];

/// Every base state the resolver is asked about.
const BASES: [BaseState; 5] = [
    BaseState::Rest,
    BaseState::Hovered,
    BaseState::Pressed,
    BaseState::Invalid,
    BaseState::Disabled,
];

/// Tolerance for a ratio asserted to four decimal places.
const RATIO_TOLERANCE: f32 = 1e-3;

fn palette() -> Palette {
    Palette::dark()
}

fn style(polarity: ChipPolarity, base: BaseState, focused: bool) -> ChipStyle {
    resolve_style(
        polarity,
        ChipHeight::Compact,
        VisualState { base, focused },
        &palette(),
    )
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < RATIO_TOLERANCE
}

fn ratio(a: Color, b: Color) -> f32 {
    contrast::contrast_ratio(a, b)
}

// ---------------------------------------------------------------------------
// 1. The matrix
// ---------------------------------------------------------------------------

#[test]
fn the_unselected_row_is_3_6_verbatim() {
    let p = palette();
    let rest = style(ChipPolarity::Unselected, BaseState::Rest, false);
    assert_eq!(rest.fill, p.surface_raised);
    assert_eq!(rest.text, p.text_primary);
    assert_eq!(rest.border, p.border_strong);

    let hover = style(ChipPolarity::Unselected, BaseState::Hovered, false);
    assert_eq!(hover.fill, p.surface_hover);
    assert_eq!(hover.text, p.text_strong);
}

#[test]
fn the_selected_row_is_3_6_verbatim() {
    let p = palette();
    let selected = style(ChipPolarity::Selected, BaseState::Rest, false);
    assert_eq!(selected.fill, p.accent_fill);
    assert_eq!(selected.text, p.text_strong);
    assert_eq!(selected.border, p.accent_border);
    assert_eq!(selected.glyph, p.text_strong);
    assert!(selected.shows_check);
}

#[test]
fn the_unselected_polarity_never_shows_the_check() {
    for base in BASES {
        assert!(
            !style(ChipPolarity::Unselected, base, false).shows_check,
            "{base:?} must not paint a check in the unselected row",
        );
    }
}

#[test]
fn the_pressed_cells_take_the_crate_third_rung() {
    let p = palette();
    let unselected = style(ChipPolarity::Unselected, BaseState::Pressed, false);
    assert_eq!(unselected.fill, p.surface_active);
    assert_eq!(unselected.border, p.accent_border);
    assert_eq!(unselected.text, p.text_strong);

    let selected = style(ChipPolarity::Selected, BaseState::Pressed, false);
    assert_eq!(selected.fill, p.accent_fill_active);
    assert_eq!(selected.border, p.accent_border);
    assert_eq!(selected.text, p.text_strong);
}

#[test]
fn invalid_shares_the_rest_cell_in_both_polarities() {
    for polarity in POLARITIES {
        assert_eq!(
            style(polarity, BaseState::Invalid, false),
            style(polarity, BaseState::Rest, false),
            "{polarity:?} has nothing to validate, so Invalid must not repaint it",
        );
    }
}

#[test]
fn both_polarities_disabled_is_the_crate_disabled_trio() {
    let p = palette();
    for polarity in POLARITIES {
        let disabled = style(polarity, BaseState::Disabled, false);
        assert_eq!(disabled.fill, p.surface_raised, "{polarity:?}");
        assert_eq!(disabled.border, p.border_subtle, "{polarity:?}");
        assert_eq!(disabled.text, p.text_disabled, "{polarity:?}");
    }
}

#[test]
fn a_disabled_selected_chip_keeps_its_check_glyph() {
    // §3.6 makes the `✓` the selected chip's mark. Dropping it to
    // `text_disabled` would have the chip report an unselected state it
    // does not have, so the glyph stays `text.strong` while the label
    // goes quiet.
    let p = palette();
    let disabled = style(ChipPolarity::Selected, BaseState::Disabled, false);
    assert_eq!(disabled.glyph, p.text_strong);
    assert_eq!(disabled.text, p.text_disabled);
    assert!(disabled.shows_check);
}

#[test]
fn the_selected_fill_cannot_survive_into_the_disabled_state() {
    // The reason the disabled selected chip leaves `accent.fill`: the
    // disabled label on that fill is unreadable.
    let p = palette();
    let ratio = ratio(p.text_disabled, p.accent_fill);
    assert!(
        ratio < 2.0,
        "text_disabled on accent.fill is {ratio:.4}:1 — the fill must drop",
    );
    assert!(!contrast::meets_text_contrast(
        p.text_disabled,
        p.accent_fill
    ));
}

#[test]
fn the_border_width_is_constant_across_every_cell() {
    for polarity in POLARITIES {
        for base in BASES {
            assert_eq!(
                style(polarity, base, false).border_width,
                shape::BORDER_HAIRLINE,
                "{polarity:?} / {base:?} must not thicken its border",
            );
        }
    }
}

#[test]
fn every_cell_resolves_deterministically() {
    for polarity in POLARITIES {
        for base in BASES {
            let a = style(polarity, base, true);
            let b = style(polarity, base, true);
            assert_eq!(a, b, "{polarity:?} / {base:?} must be a pure function");
        }
    }
}

// ---------------------------------------------------------------------------
// 2. Focus, the fill lock and the perimeter class
// ---------------------------------------------------------------------------

#[test]
fn the_focus_ring_is_additive_and_touches_no_other_channel() {
    for polarity in POLARITIES {
        for base in BASES {
            let bare = style(polarity, base, false);
            let ringed = style(polarity, base, true);
            assert_eq!(bare.focus_ring_width, 0.0);
            assert_eq!(bare.focus_ring_color, Color::TRANSPARENT);
            assert_eq!(ringed.focus_ring_width, FOCUS_RING_WIDTH);
            assert_eq!(
                ringed.fill, bare.fill,
                "the ring must not have moved the fill",
            );
            assert_eq!(ringed.border, bare.border);
            assert_eq!(ringed.text, bare.text);
        }
    }
}

#[test]
fn the_ring_is_computed_from_the_fill_it_lands_on() {
    let p = palette();
    // Unselected rest sits on `surface.raised`, where the accent ring
    // clears the floor outright.
    let unselected = style(ChipPolarity::Unselected, BaseState::Rest, true);
    assert_eq!(
        unselected.focus_ring_color,
        focus::resolve_ring_color_for_background(p.surface_raised, &p)
    );
    assert_eq!(unselected.focus_ring_color, p.accent_border);

    // Selected sits on `accent.fill`, where the accent ring is only
    // 1.6098:1 and the rule falls through to `text.strong`.
    let selected = style(ChipPolarity::Selected, BaseState::Rest, true);
    assert_eq!(selected.focus_ring_color, p.text_strong);
    assert!(ratio(p.accent_border, p.accent_fill) < 3.0);
}

#[test]
fn the_selected_fill_carries_the_rule_4_lock() {
    let p = palette();
    let selected = style(ChipPolarity::Selected, BaseState::Rest, false);
    let lock = contrast::fill_lock(selected.fill, selected.text, &p);
    assert!(
        lock.is_compliant(),
        "the selected chip's fill/foreground pair must satisfy rule 4, got {lock:?}",
    );
    assert_eq!(lock, FillLock::Locked);
}

#[test]
fn the_unselected_fill_is_not_a_locked_solid() {
    let p = palette();
    let unselected = style(ChipPolarity::Unselected, BaseState::Rest, false);
    let lock = contrast::fill_lock(unselected.fill, unselected.text, &p);
    assert_eq!(
        lock,
        FillLock::NotSolidFill,
        "a surface is not a locked solid fill, so rule 4 makes no demand here",
    );
    assert!(
        !lock.governed(),
        "the lock must report that it did not apply, not that it was waived",
    );
    assert!(contrast::meets_text_contrast(
        unselected.text,
        unselected.fill
    ));
}

#[test]
fn the_disabled_label_sits_inside_the_disabled_band() {
    for polarity in POLARITIES {
        let disabled = style(polarity, BaseState::Disabled, false);
        assert!(
            contrast::meets_disabled_text_band(disabled.text, disabled.fill),
            "{polarity:?} disabled label must land in the 4.0–6.0 band",
        );
    }
}

#[test]
fn the_perimeter_classifies_by_token_identity() {
    let p = palette();
    assert_eq!(
        chip_chrome_class(&style(ChipPolarity::Unselected, BaseState::Rest, false), &p),
        ChromeClass::Judged,
    );
    assert_eq!(
        chip_chrome_class(&style(ChipPolarity::Selected, BaseState::Rest, false), &p),
        ChromeClass::Judged,
    );
    assert_eq!(
        chip_chrome_class(
            &style(ChipPolarity::Selected, BaseState::Hovered, false),
            &p
        ),
        ChromeClass::Judged,
    );
    assert_eq!(
        chip_chrome_class(
            &style(ChipPolarity::Selected, BaseState::Disabled, false),
            &p
        ),
        ChromeClass::RestingDivider,
        "a disabled chip's border.subtle edge is not indicator material",
    );
}

#[test]
fn no_chip_resolves_to_a_coincident_perimeter() {
    for polarity in POLARITIES {
        for base in BASES {
            let s = style(polarity, base, false);
            assert_ne!(
                s.border, s.fill,
                "{polarity:?} / {base:?} drew a border coincident with its fill, \
                 which §3.6 never asks for",
            );
        }
    }
}

#[test]
fn every_judged_perimeter_clears_the_chrome_floor_against_its_mount() {
    let p = palette();
    let mount = p.surface_raised;
    for polarity in POLARITIES {
        for base in BASES {
            let s = style(polarity, base, false);
            let class = chip_chrome_class(&s, &p);
            assert!(
                contrast::meets_chrome_contrast_as(class, s.border, mount),
                "{polarity:?} / {base:?} perimeter {class:?} fails against surface.raised",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3. The recomputed contrast table
// ---------------------------------------------------------------------------

#[test]
fn every_ratio_3_6_prints_is_reproduced_from_raw_srgb() {
    let p = palette();
    let pairs: [(&str, Color, Color, f32); 3] = [
        (
            "text.primary / surface.raised",
            p.text_primary,
            p.surface_raised,
            12.3755,
        ),
        (
            "border.strong / surface.raised",
            p.border_strong,
            p.surface_raised,
            4.2379,
        ),
        (
            "text.strong / accent.fill",
            p.text_strong,
            p.accent_fill,
            4.8020,
        ),
    ];
    for (name, a, b, expected) in pairs {
        let got = ratio(a, b);
        assert!(
            close(got, expected),
            "{name}: recomputed {got:.4}, expected {expected:.4}",
        );
    }
}

#[test]
fn the_supporting_figures_the_resolver_leans_on_are_what_they_claim() {
    let p = palette();
    let table: [(&str, Color, Color, f32); 6] = [
        (
            "accent.border / surface.raised",
            p.accent_border,
            p.surface_raised,
            6.2727,
        ),
        (
            "accent.border / surface.hover",
            p.accent_border,
            p.surface_hover,
            5.4427,
        ),
        (
            "accent.fill / surface.raised",
            p.accent_fill,
            p.surface_raised,
            3.8965,
        ),
        (
            "accent_fill_active / surface.raised",
            p.accent_fill_active,
            p.surface_raised,
            3.3082,
        ),
        (
            "text.strong / accent_fill_active",
            p.text_strong,
            p.accent_fill_active,
            5.6559,
        ),
        (
            "text_disabled / surface.raised",
            p.text_disabled,
            p.surface_raised,
            5.5643,
        ),
    ];
    for (name, a, b, expected) in table {
        let got = ratio(a, b);
        assert!(
            close(got, expected),
            "{name}: recomputed {got:.4}, expected {expected:.4}",
        );
    }
}

#[test]
fn a_selected_chip_may_not_be_mounted_on_surface_active() {
    // §3.6 names the allowed mounts — base, raised, overlay and hover —
    // and rules out surface.active by name: `accent.fill` against
    // `surface.active` is 2.9768:1, under the 3.00:1 a fill that carries
    // its own boundary owes.
    let p = palette();
    let fill = p.accent_fill;
    assert!(contrast::meets_coincident_fill_contrast(
        fill,
        p.surface_base
    ));
    assert!(contrast::meets_coincident_fill_contrast(
        fill,
        p.surface_raised
    ));
    assert!(contrast::meets_coincident_fill_contrast(
        fill,
        p.surface_overlay
    ));
    assert!(contrast::meets_coincident_fill_contrast(
        fill,
        p.surface_hover
    ));
    assert!(close(ratio(fill, p.surface_overlay), 3.6895));
    assert!(
        !contrast::meets_coincident_fill_contrast(fill, p.surface_active),
        "surface.active must stay forbidden, recomputed {}",
        ratio(fill, p.surface_active),
    );
    assert!(close(ratio(fill, p.surface_active), 2.9768));
}

#[test]
fn the_selected_chip_is_still_legible_through_its_own_perimeter() {
    // The forbidden-mount finding is about the *fill* carrying the
    // boundary. §3.6 gives the selected chip a perimeter of its own, so
    // the chip's edge is judged at 6.2727:1 even where the fill alone
    // would fail.
    let p = palette();
    let selected = style(ChipPolarity::Selected, BaseState::Rest, false);
    assert!(close(ratio(selected.border, p.surface_active), 4.7922,));
    assert!(contrast::meets_chrome_contrast(
        selected.border,
        p.surface_active
    ));
}

// ---------------------------------------------------------------------------
// 4. Shape and slots
// ---------------------------------------------------------------------------

#[test]
fn the_filter_and_input_variants_take_the_widget_radius() {
    let base = resolve_style(
        ChipPolarity::Unselected,
        ChipHeight::Bar,
        VisualState {
            base: BaseState::Rest,
            focused: false,
        },
        &palette(),
    );
    assert_eq!(base.radius, RADIUS_WIDGET);
    assert_eq!(base.radius, shape::RADIUS_WIDGET);

    let filter = with_variant(base, false, false);
    assert_eq!(filter.radius, shape::RADIUS_WIDGET);

    let input = with_variant(base, false, true);
    assert_eq!(input.radius, shape::RADIUS_WIDGET);
    assert!(input.shows_remove);
    assert_eq!(input.remove_target, REMOVE_TARGET);
}

#[test]
fn the_assist_variant_takes_the_pill_radius() {
    let base = resolve_style(
        ChipPolarity::Unselected,
        ChipHeight::Compact,
        VisualState {
            base: BaseState::Rest,
            focused: false,
        },
        &palette(),
    );
    let assist = with_variant(base, true, false);
    assert_eq!(assist.radius, RADIUS_PILL);
    assert_eq!(assist.radius, shape::RADIUS_PILL);
    assert!(!assist.shows_remove);
}

#[test]
fn with_variant_does_not_touch_the_paint_channels() {
    let base = style(ChipPolarity::Selected, BaseState::Rest, true);
    let assist = with_variant(base, true, true);
    assert_eq!(assist.fill, base.fill);
    assert_eq!(assist.border, base.border);
    assert_eq!(assist.text, base.text);
    assert_eq!(assist.glyph, base.glyph);
    assert_eq!(assist.focus_ring_color, base.focus_ring_color);
    assert_eq!(assist.height, base.height);
    assert_eq!(assist.h_padding, base.h_padding);
    assert_eq!(assist.label_gap, base.label_gap);
    assert_eq!(assist.shows_check, base.shows_check);
}

#[test]
fn resolve_model_applies_polarity_rung_and_shape_in_one_call() {
    let p = palette();
    let live = live_input();

    let assist = ChipModel::assist("a", "Assist").with_height(ChipHeight::Bar);
    let assist_style = resolve_model(&assist, live, &p);
    assert_eq!(assist_style.radius, shape::RADIUS_PILL);
    assert_eq!(assist_style.height, size::CONTROL_SM);
    assert!(!assist_style.shows_check);
    assert!(!assist_style.shows_remove);

    let filter = ChipModel::filter("f", "Filter", true);
    let filter_style = resolve_model(&filter, live, &p);
    assert_eq!(filter_style.fill, p.accent_fill);
    assert!(filter_style.shows_check);
    assert_eq!(filter_style.radius, shape::RADIUS_WIDGET);

    let input = ChipModel::input("i", "Input", true).with_height(ChipHeight::Bar);
    let input_style = resolve_model(&input, live, &p);
    assert!(input_style.shows_remove);
    assert_eq!(input_style.height, size::CONTROL_SM);
    assert!(!input_style.shows_check);
}

#[test]
fn a_disabled_model_resolves_the_disabled_cell() {
    let p = palette();
    let model = ChipModel::filter("f", "Filter", true).disabled();
    let s = resolve_model(&model, live_input(), &p);
    assert_eq!(s.fill, p.surface_raised);
    assert_eq!(s.border, p.border_subtle);
    assert_eq!(s.text, p.text_disabled);
    assert_eq!(s.glyph, p.text_strong, "the check survives the disable");
    assert_eq!(s.focus_ring_width, 0.0, "a dead control draws no ring");
}

#[test]
fn the_model_disabled_flag_overrides_a_live_looking_input() {
    // The point of `resolve_model` taking `InteractionInput`: a caller
    // cannot paint a disabled chip as live by handing over a hovered,
    // focused, enabled observation.
    let p = palette();
    let model = ChipModel::assist("a", "Assist").disabled();
    let observed = InteractionInput {
        enabled: true,
        hovered: true,
        pressed: true,
        focused: true,
        invalid: false,
    };
    let s = resolve_model(&model, observed, &p);
    assert_eq!(s.fill, p.surface_raised);
    assert_eq!(s.border, p.border_subtle);
    assert_eq!(s.focus_ring_width, 0.0);

    // The state the resolver actually ran on is Disabled, not the
    // Pressed+Focused the caller asked for.
    let state = resolve(InteractionInput {
        enabled: !model.is_disabled(),
        ..observed
    });
    assert_eq!(state.base, BaseState::Disabled);
    assert!(!state.focused);
    assert_eq!(state.snapshot_label(), "Disabled");
}

/// A live, unhovered, unfocused, valid observation.
fn live_input() -> InteractionInput {
    InteractionInput {
        enabled: true,
        hovered: false,
        pressed: false,
        focused: false,
        invalid: false,
    }
}

// ---------------------------------------------------------------------------
// 5. Geometry
// ---------------------------------------------------------------------------

#[test]
fn the_constants_are_the_tokens_3_6_names() {
    assert_eq!(COMPACT_HEIGHT, size::BAR_STATUS);
    assert_eq!(BAR_HEIGHT, size::CONTROL_SM);
    assert_eq!(H_PADDING, space::SM);
    assert_eq!(LABEL_GAP, space::XS);
    assert_eq!(REMOVE_TARGET, size::HIT_TARGET_MIN);
    assert_eq!(GLYPH_SIZE, size::ICON_SM);
    assert_eq!(BORDER_WIDTH, shape::BORDER_HAIRLINE);
    assert_eq!(FOCUS_RING_WIDTH, shape::BORDER_FOCUS);
}

#[test]
fn a_plain_chip_is_label_plus_padding() {
    let s = style(ChipPolarity::Unselected, BaseState::Rest, false);
    assert_eq!(chip_width(&s, 0.0, 50.0), 50.0 + H_PADDING * 2.0);
}

#[test]
fn a_leading_glyph_costs_its_width_plus_one_gap() {
    let s = style(ChipPolarity::Unselected, BaseState::Rest, false);
    let plain = chip_width(&s, 0.0, 50.0);
    let led = chip_width(&s, GLYPH_SIZE, 50.0);
    assert_eq!(led - plain, GLYPH_SIZE + LABEL_GAP);
}

#[test]
fn a_removable_chip_reserves_the_slot_whether_or_not_it_is_wanted() {
    let base = style(ChipPolarity::Unselected, BaseState::Rest, false);
    let plain = chip_width(&base, 0.0, 50.0);

    let removable = with_variant(base, false, true);
    let wide = chip_width(&removable, 0.0, 50.0);
    assert_eq!(wide - plain, REMOVE_TARGET + LABEL_GAP);
    assert_eq!(wide - plain, size::HIT_TARGET_MIN + space::XS);
}

#[test]
fn a_chip_with_every_slot_charges_every_one() {
    let base = style(ChipPolarity::Selected, BaseState::Rest, false);
    let full = with_variant(base, false, true);
    let width = chip_width(&full, GLYPH_SIZE, 40.0);
    assert_eq!(
        width,
        H_PADDING * 2.0 + GLYPH_SIZE + LABEL_GAP + 40.0 + LABEL_GAP + REMOVE_TARGET,
    );
}

#[test]
fn the_remove_slot_is_the_box_the_width_function_paid_for() {
    let base = style(ChipPolarity::Unselected, BaseState::Rest, false);
    let removable = with_variant(base, false, true);
    let text_width = 40.0;
    let width = chip_width(&removable, 0.0, text_width);
    let bounds = Rectangle::new(Point::new(100.0, 50.0), Size::new(width, removable.height));

    let slot = remove_slot(&removable, bounds).expect("a removable chip has a slot");
    assert_eq!(slot.width, REMOVE_TARGET);
    assert_eq!(slot.height, REMOVE_TARGET);
    assert!(
        slot.x + slot.width <= bounds.x + bounds.width - H_PADDING + f32::EPSILON,
        "the slot must sit inside the chip's trailing padding",
    );
    assert!(bounds.contains(Point::new(slot.x, slot.y)));
    assert!(bounds.contains(Point::new(
        slot.x + slot.width - 0.001,
        slot.y + slot.height - 0.001
    )));
}

#[test]
fn the_slot_is_exactly_the_chip_tall_on_the_compact_rung() {
    let base = style(ChipPolarity::Unselected, BaseState::Rest, false);
    let removable = with_variant(base, false, true);
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(120.0, COMPACT_HEIGHT));
    let slot = remove_slot(&removable, bounds).expect("removable");
    assert_eq!(
        slot.height, COMPACT_HEIGHT,
        "on the 24px rung the 24px target fills the chip's height exactly",
    );
    assert_eq!(slot.y, 0.0);
}

#[test]
fn the_slot_is_centred_on_the_bar_rung() {
    let base = resolve_style(
        ChipPolarity::Unselected,
        ChipHeight::Bar,
        VisualState {
            base: BaseState::Rest,
            focused: false,
        },
        &palette(),
    );
    let removable = with_variant(base, false, true);
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(140.0, BAR_HEIGHT));
    let slot = remove_slot(&removable, bounds).expect("removable");
    assert_eq!(slot.y, (BAR_HEIGHT - REMOVE_TARGET) / 2.0);
    assert_eq!(slot.y, 2.0);
}

#[test]
fn a_non_removable_chip_has_no_slot() {
    let s = style(ChipPolarity::Unselected, BaseState::Rest, false);
    let bounds = Rectangle::new(Point::ORIGIN, Size::new(120.0, COMPACT_HEIGHT));
    assert_eq!(remove_slot(&s, bounds), None);
}

#[test]
fn the_slot_never_escapes_a_chip_narrower_than_its_own_padding() {
    // A degenerate chip: narrower than the padding it was charged. The
    // slot must clamp to the chip's left edge rather than run off into
    // negative coordinates.
    let base = style(ChipPolarity::Unselected, BaseState::Rest, false);
    let removable = with_variant(base, false, true);
    let bounds = Rectangle::new(Point::new(10.0, 0.0), Size::new(4.0, COMPACT_HEIGHT));
    let slot = remove_slot(&removable, bounds).expect("removable");
    assert!(
        slot.x >= bounds.x,
        "the slot clamped to the chip's left edge"
    );
}

#[test]
fn border_of_carries_the_styles_three_channels() {
    let s = style(ChipPolarity::Selected, BaseState::Rest, false);
    let border = border_of(&s);
    assert_eq!(border.color, s.border);
    assert_eq!(border.width, s.border_width);
    assert_eq!(border.radius.top_left, s.radius);
}

#[test]
fn the_model_gesture_and_the_style_agree_about_the_slot() {
    // Whatever the model says is removable, the style reserves for it —
    // and the reverse. A disagreement here is a `×` painted on a chip
    // with no hit target, or the reverse.
    for variant in [
        ChipVariant::Assist,
        ChipVariant::Filter { selected: false },
        ChipVariant::Filter { selected: true },
        ChipVariant::Input { removable: false },
        ChipVariant::Input { removable: true },
    ] {
        let model = ChipModel::new("c", "Chip", variant);
        let s = resolve_model(&model, live_input(), &Palette::dark());
        assert_eq!(
            s.shows_remove,
            model.is_removable(),
            "{variant:?}: painted slot and reserved slot must agree",
        );
        assert_eq!(
            s.shows_check,
            model.is_selected(),
            "{variant:?}: painted check and selection must agree",
        );
    }
}

#[test]
fn resolve_goes_through_the_state_machine_not_a_shortcut() {
    // Pressed beats hovered, so a chip that is both shows the pressed
    // cell. Guards against a resolver that read `hovered` directly.
    let p = palette();
    let pressed = resolve(InteractionInput {
        enabled: true,
        hovered: true,
        pressed: true,
        focused: false,
        invalid: false,
    });
    let s = resolve_style(ChipPolarity::Unselected, ChipHeight::Compact, pressed, &p);
    assert_eq!(s.fill, p.surface_active);
}
