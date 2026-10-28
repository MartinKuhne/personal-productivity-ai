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

// ---------------------------------------------------------------------------
// 6. The live iced Hyperlink Widget (UIF-ATOM-001d, §3.1d)
// ---------------------------------------------------------------------------

use crate::binding::Controlled;
use crate::test_helpers::{Harness, Theme, centre, focus_one};
use iced::Font;
use iced::keyboard::Location;
use iced::keyboard::Modifiers;
use iced::keyboard::key::{Key, Named};
use iced_test::futures::futures::executor::block_on;
use iced_test::renderer::Renderer as HeadlessRenderer;

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestMsg {
    Pressed(HyperlinkTarget),
    Emitted(HyperlinkEmission),
}

fn key_press(name: Named) -> Event {
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: Key::Named(name),
        modified_key: Key::Named(name),
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: Location::Standard,
        modifiers: Modifiers::NONE,
        text: None,
        repeat: false,
    })
}

fn widget_bounds<M: Send + 'static, R: renderer::Renderer>(
    harness: &mut Harness<M, R>,
) -> Rectangle {
    let recorded = harness.recorded();
    assert_eq!(
        recorded.len(),
        1,
        "a hyperlink registers exactly one focusable node, got {recorded:?}"
    );
    recorded[0].1
}

const FAR_AWAY: Point = Point::new(10_000.0, 10_000.0);

#[test]
fn test_standalone_and_controlled_creation() {
    // Standalone
    let link = Hyperlink::<TestMsg>::standalone("Docs", "https://docs.rs", "link_docs", |target| {
        Some(TestMsg::Pressed(target))
    })
    .external(true)
    .enabled(true)
    .mount(Color::BLACK)
    .palette(Palette::dark());

    assert_eq!(link.label(), "Docs");
    assert_eq!(link.uri(), "https://docs.rs");
    assert!(link.is_external());
    assert!(link.is_enabled());
    assert_eq!(link.focus_id().as_str(), "link_docs");
    assert_eq!(link.widget_id(), &crate::bridge::widget_id(link.focus_id()));

    // Controlled
    let model = HyperlinkModel::external("About", "/about").disabled();
    let ctl_link =
        Hyperlink::<TestMsg>::controlled(Controlled::new(&model), "link_about", |emission| {
            Some(TestMsg::Emitted(emission))
        });

    assert_eq!(ctl_link.label(), "About");
    assert_eq!(ctl_link.uri(), "/about");
    assert!(ctl_link.is_external());
    assert!(!ctl_link.is_enabled());
    assert_eq!(ctl_link.focus_id().as_str(), "link_about");
}

#[test]
fn test_mouse_hover_press_and_release_activation() {
    let mut harness = Harness::headless(|| {
        Hyperlink::standalone("Click me", "https://example.com", "link_click", |target| {
            Some(TestMsg::Pressed(target))
        })
        .into()
    });

    let bounds = widget_bounds(&mut harness);
    let over = centre(bounds);

    // Hover does not emit press message
    let hover_msgs = harness.hover(over);
    assert!(hover_msgs.is_empty());

    // Press does not emit press message
    let press_msgs = harness.press(over);
    assert!(press_msgs.is_empty());

    // Release over bounds emits Pressed message
    let release_msgs = harness.release(over);
    assert_eq!(
        release_msgs,
        vec![TestMsg::Pressed(HyperlinkTarget::new(
            "https://example.com",
            false
        ))]
    );
}

#[test]
fn test_mouse_drag_out_cancels_activation() {
    let mut harness = Harness::headless(|| {
        Hyperlink::standalone("Click me", "https://example.com", "link_cancel", |target| {
            Some(TestMsg::Pressed(target))
        })
        .into()
    });

    let bounds = widget_bounds(&mut harness);
    let over = centre(bounds);

    harness.hover(over);
    harness.press(over);

    // Release far away from bounds
    let release_msgs = harness.release(FAR_AWAY);
    assert!(
        release_msgs.is_empty(),
        "releasing outside the bounds must not activate the hyperlink"
    );
}

#[test]
fn test_controlled_mouse_activation() {
    let model = HyperlinkModel::external("Open External", "https://iced.rs");
    let mut harness = Harness::headless(|| {
        Hyperlink::controlled(Controlled::new(&model), "link_ext", |emission| {
            Some(TestMsg::Emitted(emission))
        })
        .into()
    });

    let bounds = widget_bounds(&mut harness);
    let over = centre(bounds);

    harness.hover(over);
    harness.press(over);
    let messages = harness.release(over);

    assert_eq!(messages.len(), 1);
    match &messages[0] {
        TestMsg::Emitted(HyperlinkEmission::Opened(commit)) => {
            assert_eq!(
                commit.value(),
                &HyperlinkTarget::new("https://iced.rs", true)
            );
        }
        other => panic!("expected Opened emission, got {other:?}"),
    }
}

#[test]
fn test_keyboard_enter_and_space_activation_when_focused() {
    let mut harness = Harness::headless(|| {
        Hyperlink::standalone(
            "Key link",
            "https://example.com/key",
            "link_key",
            |target| Some(TestMsg::Pressed(target)),
        )
        .into()
    });

    focus_one(&mut harness, "link_key");

    // Enter activates
    let enter_msgs = harness.pointer(&[key_press(Named::Enter)], mouse::Cursor::Unavailable);
    assert_eq!(
        enter_msgs,
        vec![TestMsg::Pressed(HyperlinkTarget::new(
            "https://example.com/key",
            false
        ))]
    );

    // Space activates
    let space_msgs = harness.pointer(&[key_press(Named::Space)], mouse::Cursor::Unavailable);
    assert_eq!(
        space_msgs,
        vec![TestMsg::Pressed(HyperlinkTarget::new(
            "https://example.com/key",
            false
        ))]
    );
}

#[test]
fn test_keyboard_ignored_when_unfocused() {
    let mut harness = Harness::headless(|| {
        Hyperlink::standalone(
            "Unfocused",
            "https://example.com",
            "link_unfocused",
            |target| Some(TestMsg::Pressed(target)),
        )
        .into()
    });

    let enter_msgs = harness.pointer(&[key_press(Named::Enter)], mouse::Cursor::Unavailable);
    assert!(
        enter_msgs.is_empty(),
        "unfocused link must not handle Enter"
    );

    let space_msgs = harness.pointer(&[key_press(Named::Space)], mouse::Cursor::Unavailable);
    assert!(
        space_msgs.is_empty(),
        "unfocused link must not handle Space"
    );
}

#[test]
fn test_disabled_link_ignores_clicks_and_keyboard() {
    let mut harness = Harness::headless(|| {
        Hyperlink::standalone(
            "Disabled",
            "https://example.com",
            "link_disabled",
            |target| Some(TestMsg::Pressed(target)),
        )
        .enabled(false)
        .into()
    });

    // A disabled link does not register with the focus router
    assert!(
        harness.recorded().is_empty(),
        "a disabled link must not register with the focus router"
    );

    let over = Point::new(10.0, 10.0);

    // Mouse click does nothing
    harness.hover(over);
    harness.press(over);
    let release_msgs = harness.release(over);
    assert!(
        release_msgs.is_empty(),
        "disabled link must not emit on click"
    );

    // Keyboard keys do nothing
    let key_msgs = harness.pointer(&[key_press(Named::Enter)], mouse::Cursor::Unavailable);
    assert!(
        key_msgs.is_empty(),
        "disabled link must not emit on key press"
    );
}

#[test]
fn test_hover_emits_uri_on_enter_and_none_on_exit() {
    let model = HyperlinkModel::new("Hover me", "https://preview.url");
    let mut harness = Harness::headless(|| {
        Hyperlink::controlled(Controlled::new(&model), "link_hover", |emission| {
            Some(TestMsg::Emitted(emission))
        })
        .into()
    });

    let bounds = widget_bounds(&mut harness);
    let over = centre(bounds);

    // Cursor enters: emits HoverUri(Some(...))
    let enter_msgs = harness.hover(over);
    assert_eq!(
        enter_msgs,
        vec![TestMsg::Emitted(HyperlinkEmission::HoverUri(Some(
            "https://preview.url".to_owned()
        )))]
    );

    // Cursor leaves: emits HoverUri(None)
    let exit_msgs = harness.hover(FAR_AWAY);
    assert_eq!(
        exit_msgs,
        vec![TestMsg::Emitted(HyperlinkEmission::HoverUri(None))]
    );
}

#[test]
fn test_disabled_controlled_link_does_not_emit_hover() {
    let model = HyperlinkModel::new("Disabled hover", "https://disabled.url").disabled();
    let mut harness = Harness::headless(|| {
        Hyperlink::controlled(Controlled::new(&model), "link_dis_hover", |emission| {
            Some(TestMsg::Emitted(emission))
        })
        .into()
    });

    assert!(
        harness.recorded().is_empty(),
        "a disabled controlled link must not register with focus"
    );

    let over = Point::new(10.0, 10.0);
    let enter_msgs = harness.hover(over);
    assert!(
        enter_msgs.is_empty(),
        "a disabled controlled link must not emit a hover uri preview"
    );
}

#[test]
fn test_mouse_interaction_pointer_when_enabled() {
    let mut harness = Harness::headless(|| {
        Hyperlink::standalone("Pointer", "https://example.com", "link_ptr", |target| {
            Some(TestMsg::Pressed(target))
        })
        .into()
    });

    let bounds = widget_bounds(&mut harness);
    let over = centre(bounds);

    let renderer = block_on(<HeadlessRenderer as renderer::Headless>::new(
        Font::with_name("Fira Sans"),
        Pixels(16.0),
        None,
    ))
    .unwrap();

    let mut link =
        Hyperlink::<TestMsg>::standalone("Pointer", "https://example.com", "link_ptr", |_| None);
    let tree = Tree {
        tag: tree::Tag::of::<HyperlinkState>(),
        state: tree::State::new(HyperlinkState::new()),
        children: Vec::new(),
    };
    let layout_node = layout::Node::new(bounds.size());
    let layout = Layout::new(&layout_node);

    assert_eq!(
        Widget::<TestMsg, Theme, HeadlessRenderer>::mouse_interaction(
            &link,
            &tree,
            layout,
            mouse::Cursor::Available(over),
            &bounds,
            &renderer,
        ),
        mouse::Interaction::Pointer
    );

    assert_eq!(
        Widget::<TestMsg, Theme, HeadlessRenderer>::mouse_interaction(
            &link,
            &tree,
            layout,
            mouse::Cursor::Available(FAR_AWAY),
            &bounds,
            &renderer,
        ),
        mouse::Interaction::None
    );

    // Disabled link
    link = link.enabled(false);
    assert_eq!(
        Widget::<TestMsg, Theme, HeadlessRenderer>::mouse_interaction(
            &link,
            &tree,
            layout,
            mouse::Cursor::Available(over),
            &bounds,
            &renderer,
        ),
        mouse::Interaction::None
    );
}

#[test]
fn test_drawing_through_headless_renderer_in_all_states() {
    let p = palette();
    for enabled in [true, false] {
        for focused in [false, true] {
            for mount in [p.surface_base, p.surface_overlay] {
                let mut harness = Harness::headless(move || {
                    let link = Hyperlink::<TestMsg>::standalone(
                        "Test Link",
                        "https://example.com",
                        "link_render",
                        |_| None,
                    )
                    .enabled(enabled)
                    .mount(mount);

                    link.into()
                });

                if focused && enabled {
                    focus_one(&mut harness, "link_render");
                }

                // Render at rest
                harness.render();

                // Render while hovered
                let over = Point::new(10.0, 10.0);
                harness.hover(over);
                harness.render();

                // Render while pressed
                harness.press(over);
                harness.render();

                // Release
                harness.release(over);
                harness.render();
            }
        }
    }
}

#[test]
fn test_layers_match_visual_states() {
    let p = palette();
    let bounds = Rectangle::new(Point::ORIGIN, control_size(60.0));

    // Rest: no pill, no ring, underlined
    let rest_style = resolve_style(REST, p.surface_base, &p);
    assert!(pill_layer(bounds, &rest_style).is_none());
    assert!(focus_ring_layer(bounds, &rest_style).is_none());
    assert!(underline_layer(bounds, &rest_style).is_some());

    // Hovered: pill present, no ring, underlined
    let hover_style = resolve_style(HOVERED, p.surface_base, &p);
    let pill = pill_layer(bounds, &hover_style).expect("hovered link must have pill layer");
    assert_eq!(pill.fill, p.surface_hover);
    assert_eq!(pill.bounds, bounds);
    assert!(focus_ring_layer(bounds, &hover_style).is_none());
    assert!(underline_layer(bounds, &hover_style).is_some());

    // Rest + Focused: no pill, ring present, underlined
    let focused_style = resolve_style(REST_FOCUSED, p.surface_base, &p);
    assert!(pill_layer(bounds, &focused_style).is_none());
    let ring = focus_ring_layer(bounds, &focused_style).expect("focused link must have focus ring");
    assert_eq!(ring.bounds, bounds.expand(FOCUS_RING_WIDTH));
    assert_eq!(ring.border.width, FOCUS_RING_WIDTH);
    assert_eq!(
        ring.border.color,
        crate::widget::focus::resolve_ring_color_for_background(p.surface_base, &p)
    );
    assert!(underline_layer(bounds, &focused_style).is_some());

    // Disabled: no pill, no ring, NO underline
    let disabled_style = resolve_style(DISABLED, p.surface_base, &p);
    assert!(pill_layer(bounds, &disabled_style).is_none());
    assert!(focus_ring_layer(bounds, &disabled_style).is_none());
    assert!(underline_layer(bounds, &disabled_style).is_none());
}
