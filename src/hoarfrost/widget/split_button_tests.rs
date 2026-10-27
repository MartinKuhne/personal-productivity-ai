//! Tests for the split-button geometry and paint resolver —
//! `UIF-ATOM-001c`, §3.1c.
//!
//! Six groups, in order:
//!
//! 1. **Zone resolution.** Where the divider is, which half owns its
//!    pixel, and the outside answer.
//! 2. **Composition.** Each half *is* the button resolver's answer for
//!    its own state, for both variants.
//! 3. **Half independence.** The resolver cannot put one half's rung on
//!    the other — the negative cases the spec's two-zone claim rests on.
//! 4. **Chevron and divider colour.** The glyph takes the half's own
//!    foreground; the divider is resolved per variant, as §3.1c now
//!    prescribes.
//! 5. **Geometry.** Widths, boxes, and the tiling invariant that every
//!    x resolves to the half whose bounds contain it.
//! 6. **Contrast.** Every ratio the module doc quotes, recomputed from
//!    the raw sRGB through the crate's own `contrast_ratio`.
//! 7. **Live widget.** Controlled creation, hover, clicks, menu toggle,
//!    disabled gating, keyboard navigation, and headless rendering.

use super::*;
use crate::state::BaseState;
use crate::test_helpers::{Harness, focus_one};
use crate::token::contrast::contrast_ratio;
use crate::widget::split_button_model::SplitMenuRow;
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

const VARIANTS: [ButtonVariant; 2] = [ButtonVariant::Primary, ButtonVariant::Secondary];

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
}

// ---------------------------------------------------------------------------
// 1. Zone resolution
// ---------------------------------------------------------------------------

#[test]
fn the_chevron_zone_is_the_trailing_28_pixels() {
    let box_ = rect(0.0, 0.0, 200.0, 32.0);
    assert_eq!(divider_x(200.0), 172.0);
    assert_eq!(menu_zone(box_), rect(172.0, 0.0, 28.0, 32.0));
    assert_eq!(primary_zone(box_), rect(0.0, 0.0, 172.0, 32.0));
}

#[test]
fn a_point_left_of_the_divider_is_the_primary_half() {
    assert_eq!(zone_at(0.0, 200.0), SplitZone::Primary);
    assert_eq!(zone_at(100.0, 200.0), SplitZone::Primary);
    assert_eq!(zone_at(171.9, 200.0), SplitZone::Primary);
}

#[test]
fn the_dividers_own_pixel_belongs_to_the_chevron_half() {
    // The reversible action wins the ambiguous pixel: a 1px aim error
    // should open a menu, not fire the primary command.
    assert_eq!(zone_at(172.0, 200.0), SplitZone::Menu);
    assert_eq!(zone_at(199.9, 200.0), SplitZone::Menu);
}

#[test]
fn a_point_outside_the_control_is_outside_and_not_a_clamp() {
    assert_eq!(zone_at(-1.0, 200.0), SplitZone::Outside);
    assert_eq!(zone_at(200.0, 200.0), SplitZone::Outside);
    assert_eq!(zone_at(999.0, 200.0), SplitZone::Outside);
    assert!(!SplitZone::Outside.is_hit());
    assert!(SplitZone::Primary.is_hit());
    assert!(SplitZone::Menu.is_hit());
}

#[test]
fn a_control_narrower_than_the_chevron_zone_never_reports_a_negative_divider() {
    // Degenerate width: the divider clamps to 0 so the whole control is
    // the chevron half rather than the zones overlapping into nonsense.
    assert_eq!(divider_x(10.0), 0.0);
    assert_eq!(zone_at(0.0, 10.0), SplitZone::Menu);
}

// ---------------------------------------------------------------------------
// 2. Composition
// ---------------------------------------------------------------------------

#[test]
fn each_half_is_the_button_resolver_answer_for_its_own_state() {
    let p = palette();
    for variant in VARIANTS {
        for primary in [BaseState::Rest, BaseState::Hovered, BaseState::Pressed] {
            for menu in [BaseState::Rest, BaseState::Hovered, BaseState::Pressed] {
                let style = resolve_style(
                    variant,
                    VisualState {
                        base: primary,
                        focused: false,
                    },
                    VisualState {
                        base: menu,
                        focused: false,
                    },
                    &p,
                );
                assert_eq!(
                    style.primary,
                    button::resolve_style(variant, st(primary), &p),
                    "primary half drifted from the button atom for {variant:?}/{primary:?}"
                );
                assert_eq!(
                    style.menu,
                    button::resolve_style(variant, st(menu), &p),
                    "menu half drifted from the button atom for {variant:?}/{menu:?}"
                );
            }
        }
    }
}

#[test]
fn focus_is_additive_on_both_halves_exactly_as_it_is_on_a_button() {
    let p = palette();
    for variant in VARIANTS {
        let bare = resolve_style(variant, REST, REST, &p);
        let ringed = resolve_style(variant, REST_FOCUSED, REST_FOCUSED, &p);
        assert_eq!(bare.primary.background, ringed.primary.background);
        assert_eq!(bare.menu.background, ringed.menu.background);
        assert!(ringed.primary.focus_ring_width > 0.0);
        assert!(ringed.menu.focus_ring_width > 0.0);
    }
}

// ---------------------------------------------------------------------------
// 3. Half independence
// ---------------------------------------------------------------------------

#[test]
fn hovering_the_label_leaves_the_chevron_at_rest() {
    let p = palette();
    for variant in VARIANTS {
        let style = resolve_style(variant, HOVERED, REST, &p);
        assert_eq!(style.menu, button::resolve_style(variant, REST, &p));
        assert_ne!(style.primary, style.menu);
    }
}

#[test]
fn hovering_the_chevron_leaves_the_label_at_rest() {
    let p = palette();
    for variant in VARIANTS {
        let style = resolve_style(variant, REST, HOVERED, &p);
        assert_eq!(style.primary, button::resolve_style(variant, REST, &p));
        assert_ne!(style.primary, style.menu);
    }
}

#[test]
fn pressing_one_half_never_presses_the_other() {
    let p = palette();
    for variant in VARIANTS {
        let label = resolve_style(variant, PRESSED, REST, &p);
        let chevron = resolve_style(variant, REST, PRESSED, &p);
        assert_eq!(label.menu, button::resolve_style(variant, REST, &p));
        assert_eq!(chevron.primary, button::resolve_style(variant, REST, &p));
    }
}

#[test]
fn a_disabled_control_puts_both_halves_in_the_disabled_cell() {
    let p = palette();
    for variant in VARIANTS {
        let style = resolve_style(variant, DISABLED, DISABLED, &p);
        assert_eq!(style.primary.background, style.menu.background);
        assert_eq!(style.primary.foreground, style.menu.foreground);
    }
}

// ---------------------------------------------------------------------------
// 4. Chevron and divider colour
// ---------------------------------------------------------------------------

#[test]
fn the_chevron_is_the_menu_halves_own_foreground() {
    let p = palette();
    for variant in VARIANTS {
        for menu in [BaseState::Rest, BaseState::Hovered, BaseState::Pressed] {
            let style = resolve_style(variant, REST, st(menu), &p);
            assert_eq!(chevron_color(&style), style.menu.foreground);
        }
    }
}

#[test]
fn the_chevron_is_never_the_muted_glyph_on_a_primary_fill() {
    // §3.4's combobox trigger paints its glyph `text.muted` at rest. That
    // precedent cannot travel here: on `accent.fill` it is 1.84:1.
    let p = palette();
    let style = resolve_style(ButtonVariant::Primary, REST, REST, &p);
    assert_ne!(chevron_color(&style), p.text_muted);
    assert!(
        contrast_ratio(chevron_color(&style), p.accent_fill) >= 4.5,
        "chevron on the primary fill must clear AA text"
    );
}

#[test]
fn the_divider_is_resolved_per_variant() {
    // §3.1c gives each variant its own seam token; a single border token
    // is visible on one fill and invisible on the other.
    let p = palette();
    for (primary, menu) in [(REST, REST), (HOVERED, PRESSED), (PRESSED, HOVERED)] {
        let style = resolve_style(ButtonVariant::Primary, primary, menu, &p);
        assert_eq!(style.divider, p.text_strong);
        assert_eq!(style.divider, divider_color(ButtonVariant::Primary, &p));

        let style = resolve_style(ButtonVariant::Secondary, primary, menu, &p);
        assert_eq!(style.divider, p.border_strong);
        assert_eq!(style.divider, divider_color(ButtonVariant::Secondary, &p));
    }
}

#[test]
fn the_seam_never_falls_back_to_a_token_that_is_invisible_on_its_own_fill() {
    // The Primary half must not wear the Secondary seam, and vice versa.
    let p = palette();
    assert_ne!(
        divider_color(ButtonVariant::Primary, &p),
        divider_color(ButtonVariant::Secondary, &p)
    );
    assert!(
        contrast_ratio(divider_color(ButtonVariant::Primary, &p), p.accent_fill) >= 3.0,
        "the Primary seam must clear the 3:1 chrome floor on accent.fill"
    );
    assert!(
        contrast_ratio(
            divider_color(ButtonVariant::Secondary, &p),
            p.surface_raised
        ) >= 3.0,
        "the Secondary seam must clear the 3:1 chrome floor on surface.raised"
    );
}

// ---------------------------------------------------------------------------
// 5. Geometry
// ---------------------------------------------------------------------------

#[test]
fn the_control_width_charges_the_chevron_zone_in_full() {
    assert_eq!(control_width(100.0), 100.0 + 12.0 * 2.0 + 28.0);
    assert_eq!(control_width(0.0), 52.0);
    assert_eq!(control_width(-50.0), control_width(0.0));
}

#[test]
fn widening_the_label_never_moves_the_chevron_zones_width() {
    for label in [40.0, 120.0, 400.0] {
        let box_ = rect(0.0, 0.0, control_width(label), HEIGHT_STANDARD);
        assert_eq!(menu_zone(box_).width, CHEVRON_ZONE_WIDTH);
    }
}

#[test]
fn the_label_box_sits_inside_the_primary_padding() {
    let box_ = rect(0.0, 0.0, control_width(100.0), HEIGHT_STANDARD);
    let label = label_bounds(primary_zone(box_));
    assert_eq!(label.x, PRIMARY_PADDING);
    assert_eq!(label.width, 100.0);
}

#[test]
fn the_chevron_glyph_is_centred_in_its_zone() {
    let box_ = rect(10.0, 4.0, control_width(100.0), HEIGHT_STANDARD);
    let glyph = chevron_bounds(menu_zone(box_));
    let zone = menu_zone(box_);
    let centre = |v: Rectangle| (v.x + v.width / 2.0, v.y + v.height / 2.0);
    assert_eq!(centre(glyph), centre(zone));
    assert_eq!(glyph.width, size::ICON_SM);
}

#[test]
fn every_x_resolves_to_the_half_whose_bounds_contain_it() {
    let box_ = rect(0.0, 0.0, control_width(88.0), HEIGHT_COMPACT);
    let primary = primary_zone(box_);
    let menu = menu_zone(box_);
    let mut x = 0.0;
    while x < box_.width {
        let zone = zone_at(x, box_.width);
        match zone {
            SplitZone::Primary => {
                assert!(x < primary.width, "{x} in Primary past the divider");
            }
            SplitZone::Menu => {
                assert!(x >= menu.x, "{x} in Menu before the zone start");
            }
            SplitZone::Outside => panic!("a point inside the box reported Outside at {x}"),
        }
        x += 0.5;
    }
}

#[test]
fn the_divider_bounds_sit_at_the_seam_one_pixel_wide() {
    let box_ = rect(0.0, 0.0, control_width(88.0), HEIGHT_STANDARD);
    let divider = divider_bounds(box_);
    assert_eq!(divider.x, divider_x(box_.width));
    assert_eq!(divider.width, DIVIDER_WIDTH);
    assert_eq!(divider.height, HEIGHT_STANDARD);
}

// ---------------------------------------------------------------------------
// 6. Contrast — every quoted figure recomputed from raw sRGB
// ---------------------------------------------------------------------------

#[test]
fn each_seam_token_measures_what_the_spec_prices() {
    let p = palette();
    // §3.1c's two seam figures, recomputed from raw sRGB.
    assert_ratio(
        contrast_ratio(divider_color(ButtonVariant::Primary, &p), p.accent_fill),
        4.8020,
    );
    assert_ratio(
        contrast_ratio(
            divider_color(ButtonVariant::Secondary, &p),
            p.surface_raised,
        ),
        4.2379,
    );
    // …and the two tokens §3.1c explicitly rejects on the accent fill,
    // pinned so a future palette change cannot quietly re-admit them.
    assert_ratio(contrast_ratio(p.border_strong, p.accent_fill), 1.0876);
    assert_ratio(contrast_ratio(p.text_muted, p.accent_fill), 1.8363);
    assert!(
        contrast_ratio(p.border_strong, p.accent_fill) < 3.0,
        "border.strong must stay a rejected Primary seam"
    );
    // The Secondary seam stays legible across the Secondary rungs.
    assert_ratio(contrast_ratio(p.border_strong, p.surface_base), 4.5062);
    assert_ratio(contrast_ratio(p.border_strong, p.surface_hover), 3.6772);
}

#[test]
fn the_primary_chevron_measures_what_the_module_doc_claims() {
    let p = palette();
    let style = resolve_style(ButtonVariant::Primary, REST, REST, &p);
    assert_ratio(contrast_ratio(chevron_color(&style), p.accent_fill), 4.8020);
}

#[test]
fn the_secondary_chevron_is_legible_on_every_rung_it_sits_on() {
    let p = palette();
    for base in [BaseState::Rest, BaseState::Hovered, BaseState::Pressed] {
        let style = resolve_style(ButtonVariant::Secondary, REST, st(base), &p);
        let ratio = contrast_ratio(chevron_color(&style), style.menu.background);
        assert!(
            ratio >= 4.5,
            "secondary chevron on {base:?} is {ratio:.4}:1, below AA text"
        );
    }
}

#[test]
fn a_disabled_split_button_keeps_both_halves_in_the_disabled_band() {
    let p = palette();
    for variant in VARIANTS {
        let style = resolve_style(variant, DISABLED, DISABLED, &p);
        let ratio = contrast_ratio(style.primary.foreground, style.primary.background);
        assert!(
            (4.0..=6.0).contains(&ratio),
            "{variant:?} disabled pair is {ratio:.4}:1, outside the 4.0-6.0 disabled band"
        );
    }
}

/// Assert a recomputed ratio against the figure a module doc quotes.
fn assert_ratio(actual: f32, quoted: f32) {
    assert!(
        (actual - quoted).abs() < 0.001,
        "recomputed {actual:.4} but quoted {quoted:.4}"
    );
}

/// The same focus half, a different base rung.
fn st(base: BaseState) -> VisualState {
    VisualState {
        base,
        focused: false,
    }
}

// ---------------------------------------------------------------------------
// 7. Live widget (`UIF-ATOM-001c`)
// ---------------------------------------------------------------------------

fn leaked<ActionId>(model: SplitButtonModel<ActionId>) -> &'static SplitButtonModel<ActionId> {
    Box::leak(Box::new(model))
}

fn sample_rows() -> Vec<SplitMenuRow<String>> {
    vec![
        SplitMenuRow::new("save_as".to_string(), "Save As..."),
        SplitMenuRow::new("export".to_string(), "Export PDF"),
        SplitMenuRow::new("delete".to_string(), "Delete").destructive(),
    ]
}

fn key_press(name: Named) -> Event {
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: Key::Named(name),
        modified_key: Key::Named(name),
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
        "a split button registers exactly one focusable node, got {recorded:?}"
    );
    recorded[0].1
}

#[test]
fn test_controlled_creation_and_state_initialization() {
    let model = leaked(SplitButtonModel::primary("Save", sample_rows()));
    let split = SplitButton::<SplitButtonEmission<String>, String>::controlled(
        Controlled::new(model),
        "split_save",
        Some,
    );

    assert_eq!(split.focus_id().as_str(), "split_save");
    assert_eq!(split.widget_id(), &widget_id(split.focus_id()));
    assert_eq!(split.model().label(), "Save");
    assert_eq!(split.model().variant(), ButtonVariant::Primary);

    let tag = Widget::<SplitButtonEmission<String>, iced::Theme, HeadlessRenderer>::tag(&split);
    assert_eq!(tag, tree::Tag::of::<SplitButtonState>());

    let state = Widget::<SplitButtonEmission<String>, iced::Theme, HeadlessRenderer>::state(&split);
    let downcast = state.downcast_ref::<SplitButtonState>();
    assert_eq!(downcast.host.control().as_str(), "split_save");
    assert_eq!(downcast.host.hovered_zone(), SplitZone::Outside);
    assert_eq!(downcast.host.pressed_zone(), None);
    assert!(!downcast.host.is_menu_open());
    assert!(!downcast.focusable.is_focused());
    assert!(
        downcast
            .focus
            .group_of(&FocusId::new("split_save"))
            .is_some()
    );
}

#[test]
fn test_mouse_hover_primary_vs_menu() {
    let model = leaked(SplitButtonModel::new("Save", sample_rows()));
    let mut harness = Harness::headless(move || {
        SplitButton::controlled(Controlled::new(model), "split_hover", Some).into()
    });

    let bounds = widget_bounds(&mut harness);
    let primary_pt = Point::new(bounds.x + 10.0, bounds.y + bounds.height / 2.0);
    let menu_pt = Point::new(
        bounds.x + bounds.width - 5.0,
        bounds.y + bounds.height / 2.0,
    );
    let outside_pt = Point::new(bounds.x + bounds.width + 50.0, bounds.y + 50.0);

    // Hover is inert and emits no messages
    let msgs1 = harness.hover(primary_pt);
    assert!(msgs1.is_empty());

    let msgs2 = harness.hover(menu_pt);
    assert!(msgs2.is_empty());

    let msgs3 = harness.hover(outside_pt);
    assert!(msgs3.is_empty());
}

#[test]
fn test_click_on_primary_activates_primary_and_publishes_emission() {
    let model = leaked(SplitButtonModel::primary("Commit", sample_rows()));
    let mut harness = Harness::headless(move || {
        SplitButton::controlled(Controlled::new(model), "split_primary_act", Some).into()
    });

    let bounds = widget_bounds(&mut harness);
    let primary_pt = Point::new(bounds.x + 10.0, bounds.y + bounds.height / 2.0);

    harness.hover(primary_pt);
    let press_msgs = harness.press(primary_pt);
    assert!(
        press_msgs.is_empty(),
        "press on primary produces no immediate emission"
    );

    let release_msgs = harness.release(primary_pt);
    assert_eq!(release_msgs.len(), 1);
    assert!(matches!(
        release_msgs[0],
        SplitButtonEmission::PrimaryActivated(_)
    ));

    // Drag-out cancels
    harness.hover(primary_pt);
    harness.press(primary_pt);
    let cancel_msgs = harness.release(Point::new(bounds.x + 1000.0, bounds.y + 1000.0));
    assert_eq!(cancel_msgs.len(), 1);
    assert!(matches!(
        cancel_msgs[0],
        SplitButtonEmission::PrimaryCancelled(_)
    ));
}

#[test]
fn test_click_on_menu_toggles_menu() {
    let model = leaked(SplitButtonModel::new("Options", sample_rows()));
    let mut harness = Harness::headless(move || {
        SplitButton::controlled(Controlled::new(model), "split_menu_toggle", Some).into()
    });

    let bounds = widget_bounds(&mut harness);
    let menu_pt = Point::new(
        bounds.x + bounds.width - 5.0,
        bounds.y + bounds.height / 2.0,
    );

    // Press menu zone does nothing on press
    harness.hover(menu_pt);
    let press_msgs = harness.press(menu_pt);
    assert!(press_msgs.is_empty());

    // Release toggles menu open
    let open_msgs = harness.release(menu_pt);
    assert_eq!(open_msgs.len(), 1);
    assert_eq!(open_msgs[0], SplitButtonEmission::MenuToggled(true));

    // Second click on menu toggles it closed
    harness.press(menu_pt);
    let close_msgs = harness.release(menu_pt);
    assert_eq!(close_msgs.len(), 1);
    assert_eq!(close_msgs[0], SplitButtonEmission::MenuToggled(false));
}

#[test]
fn test_disabled_split_button_ignores_events() {
    let model = leaked(SplitButtonModel::new("Inert", sample_rows()).disabled());
    let mut harness = Harness::headless(move || {
        SplitButton::controlled(Controlled::new(model), "split_disabled", Some).into()
    });

    let primary_pt = Point::new(10.0, 10.0);
    let menu_pt = Point::new(70.0, 10.0);

    harness.hover(primary_pt);
    assert!(harness.press(primary_pt).is_empty());
    assert!(harness.release(primary_pt).is_empty());

    harness.hover(menu_pt);
    assert!(harness.press(menu_pt).is_empty());
    assert!(harness.release(menu_pt).is_empty());
}

#[test]
fn test_keyboard_primary_activation_and_menu_navigation() {
    let model = leaked(SplitButtonModel::primary("Actions", sample_rows()));
    let mut harness = Harness::headless(move || {
        SplitButton::controlled(Controlled::new(model), "split_kb", Some).into()
    });

    focus_one(&mut harness, "split_kb");

    // Enter activates primary
    let enter_msgs = harness.pointer(&[key_press(Named::Enter)], mouse::Cursor::Unavailable);
    assert_eq!(enter_msgs.len(), 1);
    assert!(matches!(
        enter_msgs[0],
        SplitButtonEmission::PrimaryActivated(_)
    ));

    // Space activates primary
    let space_msgs = harness.pointer(&[key_press(Named::Space)], mouse::Cursor::Unavailable);
    assert_eq!(space_msgs.len(), 1);
    assert!(matches!(
        space_msgs[0],
        SplitButtonEmission::PrimaryActivated(_)
    ));

    // ArrowDown opens menu
    let down_msgs = harness.pointer(&[key_press(Named::ArrowDown)], mouse::Cursor::Unavailable);
    assert_eq!(down_msgs.len(), 1);
    assert_eq!(down_msgs[0], SplitButtonEmission::MenuToggled(true));

    // Escape closes menu
    let esc_msgs = harness.pointer(&[key_press(Named::Escape)], mouse::Cursor::Unavailable);
    assert_eq!(esc_msgs.len(), 1);
    assert_eq!(esc_msgs[0], SplitButtonEmission::MenuToggled(false));
}

#[test]
fn test_headless_draw_across_states() {
    let p = palette();
    for variant in [ButtonVariant::Primary, ButtonVariant::Secondary] {
        for density in [ButtonDensity::Standard, ButtonDensity::Compact] {
            for disabled in [false, true] {
                let mut m = SplitButtonModel::new("RenderMe", sample_rows()).with_density(density);
                if variant == ButtonVariant::Primary {
                    m = SplitButtonModel::primary("RenderMe", sample_rows()).with_density(density);
                }
                if disabled {
                    m = m.disabled();
                }
                let model = leaked(m);
                let mut harness = Harness::headless(move || {
                    SplitButton::controlled(Controlled::new(model), "split_render", Some)
                        .palette(p)
                        .into()
                });

                let primary_pt = Point::new(10.0, 10.0);
                let menu_pt = Point::new(80.0, 10.0);

                // Rest render
                harness.render();

                // Hover primary
                harness.hover(primary_pt);
                harness.render();

                // Press primary
                harness.press(primary_pt);
                harness.render();
                harness.release(primary_pt);

                // Hover menu
                harness.hover(menu_pt);
                harness.render();

                // Press menu
                harness.press(menu_pt);
                harness.render();
                harness.release(menu_pt);

                if !disabled {
                    focus_one(&mut harness, "split_render");
                    harness.render();
                }
            }
        }
    }
}

#[test]
fn test_split_button_layers_and_geometry() {
    let p = palette();
    let b = rect(0.0, 0.0, 200.0, HEIGHT_STANDARD);

    let rest_style = resolve_style(ButtonVariant::Primary, REST, REST, &p);
    let p_layer = primary_background_layer(b, &rest_style);
    assert_eq!(p_layer.bounds, primary_zone(b));
    assert_eq!(p_layer.fill, rest_style.primary.background);
    assert_eq!(p_layer.border.radius.top_left, RADIUS);
    assert_eq!(p_layer.border.radius.bottom_left, RADIUS);
    assert_eq!(p_layer.border.radius.top_right, 0.0);
    assert_eq!(p_layer.border.radius.bottom_right, 0.0);

    let m_layer = menu_background_layer(b, &rest_style);
    assert_eq!(m_layer.bounds, menu_zone(b));
    assert_eq!(m_layer.fill, rest_style.menu.background);
    assert_eq!(m_layer.border.radius.top_left, 0.0);
    assert_eq!(m_layer.border.radius.bottom_left, 0.0);
    assert_eq!(m_layer.border.radius.top_right, RADIUS);
    assert_eq!(m_layer.border.radius.bottom_right, RADIUS);

    let d_layer = divider_layer(b, &rest_style);
    assert_eq!(d_layer.bounds, divider_bounds(b));
    assert_eq!(d_layer.fill, rest_style.divider);

    // Unfocused has no focus ring layer
    assert_eq!(focus_ring_layer(b, &rest_style), None);

    // Focused has focus ring layer
    let focused_style = resolve_style(ButtonVariant::Primary, REST_FOCUSED, REST_FOCUSED, &p);
    let ring =
        focus_ring_layer(b, &focused_style).expect("focused split button must have focus ring");
    assert_eq!(
        ring.bounds,
        b.expand(focused_style.primary.focus_ring_width)
    );
    assert_eq!(
        ring.border.radius.top_left,
        RADIUS + focused_style.primary.focus_ring_width
    );
    assert_eq!(ring.fill, Color::TRANSPARENT);
}
