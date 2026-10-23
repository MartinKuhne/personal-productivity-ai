//! Tests for the hyperlink resolver, geometry and focus state —
//! `UIF-ATOM-001d`, §3.1d.
//!
//! Five groups, in order:
//!
//! 1. **The four cells.** Each state's fill, label colour and underline
//!    match §3.1d row for row.
//! 2. **The underline as the non-colour cue.** Present in three cells,
//!    absent in exactly one.
//! 3. **Focus.** The ring is resolved against the mount, and the
//!    `operation::Focusable` delegation behaves like every other wrapper.
//! 4. **Geometry.** The hit target, the label box, and the underline
//!    spanning the label rather than the padding.
//! 5. **Contrast.** Every ratio the module doc quotes, recomputed from
//!    the raw sRGB — including both mounts §3.1d names, and the disabled
//!    polarity.

use super::*;
use crate::token::contrast::contrast_ratio;

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

// ---------------------------------------------------------------------------
// 1. The four cells
// ---------------------------------------------------------------------------

#[test]
fn rest_is_accent_text_on_nothing() {
    let p = palette();
    let style = resolve_style(REST, p.surface_base, &p);
    assert_eq!(style.foreground, p.accent_text);
    assert!(style.is_transparent_background());
}

#[test]
fn hover_is_strong_text_on_a_hover_pill() {
    let p = palette();
    let style = resolve_style(HOVERED, p.surface_base, &p);
    assert_eq!(style.foreground, p.text_strong);
    assert_eq!(style.background, p.surface_hover);
}

#[test]
fn pressed_is_accent_text_on_the_active_surface() {
    let p = palette();
    let style = resolve_style(PRESSED, p.surface_base, &p);
    assert_eq!(style.foreground, p.accent_text);
    assert_eq!(style.background, p.surface_active);
}

#[test]
fn disabled_is_disabled_text_on_nothing() {
    let p = palette();
    let style = resolve_style(DISABLED, p.surface_base, &p);
    assert_eq!(style.foreground, p.text_disabled);
    assert!(style.is_transparent_background());
}

#[test]
fn invalid_reads_as_rest_because_a_link_has_nothing_to_validate() {
    let p = palette();
    assert_eq!(
        resolve_style(
            VisualState {
                base: BaseState::Invalid,
                focused: false,
            },
            p.surface_base,
            &p
        ),
        resolve_style(REST, p.surface_base, &p)
    );
}

// ---------------------------------------------------------------------------
// 2. The underline as the non-colour cue
// ---------------------------------------------------------------------------

#[test]
fn the_underline_is_present_in_every_live_cell() {
    let p = palette();
    for state in [REST, HOVERED, PRESSED] {
        assert!(
            resolve_style(state, p.surface_base, &p).underlined,
            "{:?} lost its underline",
            state.base
        );
    }
}

#[test]
fn the_underline_is_absent_in_exactly_one_cell() {
    let p = palette();
    let underlined: Vec<bool> = [REST, HOVERED, PRESSED, DISABLED]
        .iter()
        .map(|state| resolve_style(*state, p.surface_base, &p).underlined)
        .collect();
    assert_eq!(underlined, vec![true, true, true, false]);
}

// ---------------------------------------------------------------------------
// 3. Focus
// ---------------------------------------------------------------------------

#[test]
fn the_ring_is_resolved_against_the_mount_not_the_links_own_fill() {
    let p = palette();
    // At rest the link paints nothing, so a ring resolved against its own
    // background would be judging transparent against transparent.
    let on_base = resolve_style(REST_FOCUSED, p.surface_base, &p);
    let on_overlay = resolve_style(REST_FOCUSED, p.surface_overlay, &p);
    assert!(on_base.focus_ring_width > 0.0);
    assert_eq!(
        on_base.focus_ring_color,
        crate::widget::focus::resolve_ring_color_for_background(p.surface_base, &p)
    );
    assert_eq!(
        on_overlay.focus_ring_color,
        crate::widget::focus::resolve_ring_color_for_background(p.surface_overlay, &p)
    );
}

#[test]
fn the_ring_is_the_spec_width_and_absent_when_unfocused() {
    let p = palette();
    assert_eq!(FOCUS_RING_WIDTH, 2.0);
    let bare = resolve_style(REST, p.surface_base, &p);
    assert_eq!(bare.focus_ring_width, 0.0);
    assert_eq!(bare.focus_ring_color, Color::TRANSPARENT);
}

#[test]
fn focus_does_not_change_the_label_or_the_underline() {
    let p = palette();
    let bare = resolve_style(REST, p.surface_base, &p);
    let ringed = resolve_style(REST_FOCUSED, p.surface_base, &p);
    assert_eq!(bare.foreground, ringed.foreground);
    assert_eq!(bare.underlined, ringed.underlined);
}

#[test]
fn the_focusable_trait_delegates_to_the_shared_focusable_state() {
    let mut state = HyperlinkState::new();
    assert!(!state.is_focused());

    <HyperlinkState as operation::Focusable>::focus(&mut state);
    assert!(state.is_focused());
    assert!(<HyperlinkState as operation::Focusable>::is_focused(&state));

    <HyperlinkState as operation::Focusable>::unfocus(&mut state);
    assert!(!state.is_focused());
}

#[test]
fn hover_and_press_never_imply_focus() {
    let mut state = HyperlinkState::new();
    state.set_hovered(true);
    state.set_pressed(true);
    assert!(!state.is_focused());
    assert!(!state.visual_state(true).focused);
}

#[test]
fn focus_without_the_pointer_is_rest_plus_ring() {
    let mut state = HyperlinkState::new();
    <HyperlinkState as operation::Focusable>::focus(&mut state);
    let resolved = state.visual_state(true);
    assert_eq!(resolved.base, BaseState::Rest);
    assert!(resolved.focused);
}

#[test]
fn a_disabled_link_holds_no_ring_even_while_the_router_says_it_is_focused() {
    let mut state = HyperlinkState::new();
    <HyperlinkState as operation::Focusable>::focus(&mut state);
    state.set_hovered(true);
    let resolved = state.visual_state(false);
    assert_eq!(resolved.base, BaseState::Disabled);
    assert!(!resolved.focused);
}

#[test]
fn pressed_beats_hovered_in_the_shared_precedence() {
    let mut state = HyperlinkState::new();
    state.set_hovered(true);
    state.set_pressed(true);
    assert_eq!(state.visual_state(true).base, BaseState::Pressed);
}

#[test]
fn release_pointer_clears_both_pointer_signals_and_keeps_focus() {
    let mut state = HyperlinkState::new();
    <HyperlinkState as operation::Focusable>::focus(&mut state);
    state.set_hovered(true);
    state.set_pressed(true);
    state.release_pointer();
    assert!(!state.is_hovered());
    assert!(!state.is_pressed());
    assert!(state.is_focused());
}

// ---------------------------------------------------------------------------
// 4. Geometry
// ---------------------------------------------------------------------------

#[test]
fn the_hit_target_is_at_least_24_tall_regardless_of_label_width() {
    for label in [0.0, 40.0, 300.0] {
        assert_eq!(control_size(label).height, MIN_HEIGHT);
    }
    assert_eq!(MIN_HEIGHT, 24.0);
}

#[test]
fn the_width_is_the_label_plus_the_two_padding_slabs() {
    assert_eq!(control_size(50.0).width, 54.0);
    assert_eq!(control_size(-10.0).width, PADDING_X * 2.0);
}

#[test]
fn the_underline_spans_the_label_not_the_padding() {
    let control = Rectangle::new(Point::new(0.0, 0.0), control_size(50.0));
    let underline = underline_bounds(control);
    assert_eq!(underline.width, 50.0);
    assert_eq!(underline.x, PADDING_X);
    assert_eq!(underline.height, UNDERLINE_WIDTH);
    assert!(underline.width < control.width);
}

#[test]
fn the_underline_sits_at_the_bottom_of_the_hit_box() {
    let control = Rectangle::new(Point::new(10.0, 100.0), control_size(80.0));
    let underline = underline_bounds(control);
    assert_eq!(
        underline.y + underline.height,
        control.y + control.height,
        "the underline must not float above the hit target's bottom edge"
    );
}

#[test]
fn the_label_box_never_extends_past_the_control() {
    let control = Rectangle::new(Point::new(0.0, 0.0), control_size(60.0));
    let label = label_bounds(control);
    assert!(label.x >= control.x);
    assert!(label.x + label.width <= control.x + control.width);
}

// ---------------------------------------------------------------------------
// 5. Contrast — every quoted figure recomputed from raw sRGB
// ---------------------------------------------------------------------------

#[test]
fn the_resting_label_measures_both_figures_3_1d_prints() {
    let p = palette();
    let style = resolve_style(REST, p.surface_base, &p);
    assert_ratio(contrast_ratio(style.foreground, p.surface_base), 10.6559);
    assert_ratio(contrast_ratio(style.foreground, p.surface_overlay), 9.4891);
}

#[test]
fn the_hovered_label_is_legible_on_its_own_pill() {
    let p = palette();
    let style = resolve_style(HOVERED, p.surface_base, &p);
    assert_ratio(contrast_ratio(style.foreground, style.background), 16.2351);
}

#[test]
fn the_pressed_label_is_legible_on_its_own_pill() {
    let p = palette();
    let style = resolve_style(PRESSED, p.surface_base, &p);
    assert_ratio(contrast_ratio(style.foreground, style.background), 7.6562);
}

#[test]
fn the_disabled_label_is_inside_the_disabled_band_on_both_mounts() {
    let p = palette();
    let style = resolve_style(DISABLED, p.surface_base, &p);
    for mount in [p.surface_base, p.surface_overlay] {
        let ratio = contrast_ratio(style.foreground, mount);
        assert!(
            (4.0..=6.0).contains(&ratio),
            "disabled link on a mount is {ratio:.4}:1, outside the 4.0-6.0 band"
        );
    }
}

#[test]
fn every_live_cell_clears_aa_text_against_what_it_is_actually_painted_on() {
    let p = palette();
    for mount in [p.surface_base, p.surface_overlay, p.surface_raised] {
        for state in [REST, HOVERED, PRESSED] {
            let style = resolve_style(state, mount, &p);
            let behind = if style.is_transparent_background() {
                mount
            } else {
                style.background
            };
            let ratio = contrast_ratio(style.foreground, behind);
            assert!(
                ratio >= 4.5,
                "{:?} on {:?} is {ratio:.4}:1, below AA text",
                state.base,
                behind
            );
        }
    }
}

/// Assert a recomputed ratio against the figure a module doc quotes.
fn assert_ratio(actual: f32, quoted: f32) {
    assert!(
        (actual - quoted).abs() < 0.001,
        "recomputed {actual:.4} but quoted {quoted:.4}"
    );
}
