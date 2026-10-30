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
//!
//! A sixth group drives the live iced `Hyperlink` through the real tree.
//! Because a headless harness pays roughly 600 ms to build iced's renderer,
//! the cases inside one claim share a single session through
//! [`Harness::set_view`], each under its own focus id so no case can inherit
//! another's hover, press or focus state. Every claim still runs: a table-driven
//! group checks each of its cells on every pass, so what drops is the number of
//! renderers and of repeated `assert!` sites, never the number of checks made.

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

/// What a cell is expected to paint behind its label.
///
/// Two of the four cells are asserted as "nothing at all" rather than as a
/// particular transparent colour, and that distinction is the claim — so it
/// is spelled out here instead of being collapsed into a colour comparison.
enum Fill {
    Nothing,
    Colour(Color),
}

// ---------------------------------------------------------------------------
// 1. The four cells
// ---------------------------------------------------------------------------

#[test]
fn each_cell_resolves_its_own_label_and_fill() {
    let p = palette();

    for (label, state, want_foreground, want_fill) in [
        ("Rest", REST, p.accent_text, Fill::Nothing),
        (
            "Hovered",
            HOVERED,
            p.text_strong,
            Fill::Colour(p.surface_hover),
        ),
        (
            "Pressed",
            PRESSED,
            p.accent_text,
            Fill::Colour(p.surface_active),
        ),
        ("Disabled", DISABLED, p.text_disabled, Fill::Nothing),
    ] {
        let style = resolve_style(state, p.surface_base, &p);

        assert_eq!(
            style.foreground, want_foreground,
            "{label}: the label colour does not match §3.1d"
        );

        match want_fill {
            Fill::Nothing => assert!(
                style.is_transparent_background(),
                "{label}: must paint no background at all"
            ),
            Fill::Colour(colour) => assert_eq!(
                style.background, colour,
                "{label}: the fill does not match §3.1d"
            ),
        }
    }
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
fn the_underline_is_the_non_colour_cue_in_exactly_three_cells() {
    let p = palette();

    for state in [REST, HOVERED, PRESSED] {
        assert!(
            resolve_style(state, p.surface_base, &p).underlined,
            "{:?} lost its underline",
            state.base
        );
    }

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
fn the_focus_ring_is_resolved_against_the_mount_and_only_appears_when_focused() {
    let p = palette();

    // At rest the link paints nothing, so a ring resolved against its own
    // background would be judging transparent against transparent.
    let on_base = resolve_style(REST_FOCUSED, p.surface_base, &p);
    let on_overlay = resolve_style(REST_FOCUSED, p.surface_overlay, &p);
    assert!(on_base.focus_ring_width > 0.0);
    assert_eq!(
        on_base.focus_ring_color,
        crate::widget::focus::resolve_ring_color_for_background(p.surface_base, &p),
        "the ring on the base mount must come from that mount"
    );
    assert_eq!(
        on_overlay.focus_ring_color,
        crate::widget::focus::resolve_ring_color_for_background(p.surface_overlay, &p),
        "the ring on the overlay mount must come from that mount"
    );

    assert_eq!(FOCUS_RING_WIDTH, 2.0);
    let bare = resolve_style(REST, p.surface_base, &p);
    assert_eq!(
        bare.focus_ring_width, 0.0,
        "an unfocused link carries no ring width"
    );
    assert_eq!(
        bare.focus_ring_color,
        Color::TRANSPARENT,
        "an unfocused link carries no ring colour"
    );

    let ringed = resolve_style(REST_FOCUSED, p.surface_base, &p);
    assert_eq!(
        bare.foreground, ringed.foreground,
        "focus must not recolour the label"
    );
    assert_eq!(
        bare.underlined, ringed.underlined,
        "focus must not move the underline"
    );
}

#[test]
fn the_focusable_state_tracks_focus_independently_of_every_other_signal() {
    // The `operation::Focusable` delegation reaches the shared state.
    {
        let mut state = HyperlinkState::new();
        assert!(!state.is_focused());

        <HyperlinkState as operation::Focusable>::focus(&mut state);
        assert!(state.is_focused());
        assert!(<HyperlinkState as operation::Focusable>::is_focused(&state));

        <HyperlinkState as operation::Focusable>::unfocus(&mut state);
        assert!(!state.is_focused(), "unfocus must give the state back");
    }

    // Hover and press are pointer facts; neither implies focus.
    {
        let mut state = HyperlinkState::new();
        state.set_hovered(true);
        state.set_pressed(true);
        assert!(!state.is_focused());
        assert!(!state.visual_state(true).focused);
    }

    // Focus without the pointer is rest plus a ring.
    {
        let mut state = HyperlinkState::new();
        <HyperlinkState as operation::Focusable>::focus(&mut state);
        let resolved = state.visual_state(true);
        assert_eq!(resolved.base, BaseState::Rest);
        assert!(resolved.focused);
    }

    // A dead control shows no ring even while the router says it is focused.
    {
        let mut state = HyperlinkState::new();
        <HyperlinkState as operation::Focusable>::focus(&mut state);
        state.set_hovered(true);
        let resolved = state.visual_state(false);
        assert_eq!(resolved.base, BaseState::Disabled);
        assert!(!resolved.focused);
    }

    // Pressed outranks hovered in the shared precedence.
    {
        let mut state = HyperlinkState::new();
        state.set_hovered(true);
        state.set_pressed(true);
        assert_eq!(state.visual_state(true).base, BaseState::Pressed);
    }

    // Releasing the pointer clears both pointer signals and keeps focus.
    {
        let mut state = HyperlinkState::new();
        <HyperlinkState as operation::Focusable>::focus(&mut state);
        state.set_hovered(true);
        state.set_pressed(true);
        state.release_pointer();
        assert!(!state.is_hovered());
        assert!(!state.is_pressed());
        assert!(state.is_focused());
    }
}

// ---------------------------------------------------------------------------
// 4. Geometry
// ---------------------------------------------------------------------------

#[test]
fn the_hit_target_the_label_box_and_the_underline_are_each_where_they_are_declared() {
    // The hit target never falls below the minimum, whatever the label is.
    for label in [0.0, 40.0, 300.0] {
        assert_eq!(
            control_size(label).height,
            MIN_HEIGHT,
            "a label {label}px wide must still get the full hit height"
        );
    }
    assert_eq!(MIN_HEIGHT, 24.0);

    // The width is the label plus the two padding slabs.
    assert_eq!(control_size(50.0).width, 54.0);
    assert_eq!(
        control_size(-10.0).width,
        PADDING_X * 2.0,
        "a negative label must not shrink the control below its padding"
    );

    // The underline spans the label, not the padding.
    let control = Rectangle::new(Point::new(0.0, 0.0), control_size(50.0));
    let underline = underline_bounds(control);
    assert_eq!(underline.width, 50.0);
    assert_eq!(underline.x, PADDING_X);
    assert_eq!(underline.height, UNDERLINE_WIDTH);
    assert!(underline.width < control.width);

    // …and it sits at the bottom of the hit box.
    let control = Rectangle::new(Point::new(10.0, 100.0), control_size(80.0));
    let underline = underline_bounds(control);
    assert_eq!(
        underline.y + underline.height,
        control.y + control.height,
        "the underline must not float above the hit target's bottom edge"
    );

    // The label box never escapes the control.
    let control = Rectangle::new(Point::new(0.0, 0.0), control_size(60.0));
    let label = label_bounds(control);
    assert!(label.x >= control.x);
    assert!(label.x + label.width <= control.x + control.width);
}

// ---------------------------------------------------------------------------
// 5. Contrast — every quoted figure recomputed from raw sRGB
// ---------------------------------------------------------------------------

#[test]
fn every_quoted_contrast_figure_recomputes_from_the_raw_srgb() {
    let p = palette();

    let rest = resolve_style(REST, p.surface_base, &p);
    assert_ratio(
        contrast_ratio(rest.foreground, p.surface_base),
        10.6559,
        "the resting label on the base mount",
    );
    assert_ratio(
        contrast_ratio(rest.foreground, p.surface_overlay),
        9.4891,
        "the resting label on the overlay mount",
    );

    let hovered = resolve_style(HOVERED, p.surface_base, &p);
    assert_ratio(
        contrast_ratio(hovered.foreground, hovered.background),
        16.2351,
        "the hovered label on its own pill",
    );

    let pressed = resolve_style(PRESSED, p.surface_base, &p);
    assert_ratio(
        contrast_ratio(pressed.foreground, pressed.background),
        7.6562,
        "the pressed label on its own pill",
    );

    let disabled = resolve_style(DISABLED, p.surface_base, &p);
    for (mount_name, mount) in [("base", p.surface_base), ("overlay", p.surface_overlay)] {
        let ratio = contrast_ratio(disabled.foreground, mount);
        assert!(
            (4.0..=6.0).contains(&ratio),
            "disabled link on the {mount_name} mount is {ratio:.4}:1, outside the \
             4.0-6.0 band"
        );
    }

    for (mount_name, mount) in [
        ("surface_base", p.surface_base),
        ("surface_overlay", p.surface_overlay),
        ("surface_raised", p.surface_raised),
    ] {
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
                "{:?} on {mount_name} is {ratio:.4}:1, below AA text",
                state.base,
            );
        }
    }
}

/// Assert a recomputed ratio against the figure a module doc quotes.
fn assert_ratio(actual: f32, quoted: f32, what: &str) {
    assert!(
        (actual - quoted).abs() < 0.001,
        "{what}: recomputed {actual:.4} but quoted {quoted:.4}"
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

/// A standalone link under the case's own focus id, so several cases can
/// share one harness without sharing hover, press or focus state.
fn standalone_link(case: &str, uri: &str) -> Hyperlink<'static, TestMsg> {
    Hyperlink::standalone("Click me", uri, format!("{case}_link"), |target| {
        Some(TestMsg::Pressed(target))
    })
}

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
fn test_mouse_activation_needs_a_press_and_a_release_inside_the_bounds() {
    // Case 1 — hovering and pressing alone fire nothing; the release over
    // the bounds is what emits.
    let mut harness =
        Harness::headless(|| standalone_link("mouse_release", "https://example.com").into());

    let bounds = widget_bounds(&mut harness);
    let over = centre(bounds);

    let hover_msgs = harness.hover(over);
    assert!(
        hover_msgs.is_empty(),
        "[mouse_release] hover does not emit a press message"
    );

    let press_msgs = harness.press(over);
    assert!(
        press_msgs.is_empty(),
        "[mouse_release] press does not emit a press message"
    );

    let release_msgs = harness.release(over);
    assert_eq!(
        release_msgs,
        vec![TestMsg::Pressed(HyperlinkTarget::new(
            "https://example.com",
            false
        ))],
        "[mouse_release] release over bounds emits Pressed"
    );

    // Case 2 — walking the pointer out before the release cancels.
    harness.set_view(|| standalone_link("mouse_cancel", "https://example.com").into());

    let bounds = widget_bounds(&mut harness);
    let over = centre(bounds);

    harness.hover(over);
    harness.press(over);

    let release_msgs = harness.release(FAR_AWAY);
    assert!(
        release_msgs.is_empty(),
        "[mouse_cancel] releasing outside the bounds must not activate the hyperlink"
    );

    // Case 3 — a disabled link answers neither the pointer nor the keyboard,
    // and never registers with the router at all.
    harness.set_view(|| {
        Hyperlink::standalone(
            "Disabled",
            "https://example.com",
            "mouse_disabled",
            |target| Some(TestMsg::Pressed(target)),
        )
        .enabled(false)
        .into()
    });

    assert!(
        harness.recorded().is_empty(),
        "[mouse_disabled] a disabled link must not register with the focus router"
    );

    let at = Point::new(10.0, 10.0);

    harness.hover(at);
    harness.press(at);
    let release_msgs = harness.release(at);
    assert!(
        release_msgs.is_empty(),
        "[mouse_disabled] disabled link must not emit on click"
    );

    let key_msgs = harness.pointer(&[key_press(Named::Enter)], mouse::Cursor::Unavailable);
    assert!(
        key_msgs.is_empty(),
        "[mouse_disabled] disabled link must not emit on key press"
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
fn test_keyboard_activation_follows_focus() {
    // Case 1 — a focused link activates on Enter and on Space.
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

    let enter_msgs = harness.pointer(&[key_press(Named::Enter)], mouse::Cursor::Unavailable);
    assert_eq!(
        enter_msgs,
        vec![TestMsg::Pressed(HyperlinkTarget::new(
            "https://example.com/key",
            false
        ))],
        "[link_key] Enter activates"
    );

    let space_msgs = harness.pointer(&[key_press(Named::Space)], mouse::Cursor::Unavailable);
    assert_eq!(
        space_msgs,
        vec![TestMsg::Pressed(HyperlinkTarget::new(
            "https://example.com/key",
            false
        ))],
        "[link_key] Space activates"
    );

    // Case 2 — an unfocused link ignores both keys.
    harness.set_view(|| {
        Hyperlink::standalone(
            "Unfocused",
            "https://example.com",
            "link_unfocused",
            |target| Some(TestMsg::Pressed(target)),
        )
        .into()
    });

    // The harness carries its cache across `set_view`, so the focus case 1
    // pushed is still in the tree. Dropping it is what a harness of this
    // case's own would have given for free, and without it the claim below
    // would be testing a link that is in fact focused.
    harness.unfocus();

    let unfocused_id = crate::bridge::widget_id(&FocusId::new("link_unfocused"));
    assert!(
        !harness.is_focused(&unfocused_id),
        "[link_unfocused] precondition: the link must be unfocused before the \
         keys are sent, got {:?}",
        harness.focused()
    );

    let enter_msgs = harness.pointer(&[key_press(Named::Enter)], mouse::Cursor::Unavailable);
    assert!(
        enter_msgs.is_empty(),
        "[link_unfocused] unfocused link must not handle Enter"
    );

    let space_msgs = harness.pointer(&[key_press(Named::Space)], mouse::Cursor::Unavailable);
    assert!(
        space_msgs.is_empty(),
        "[link_unfocused] unfocused link must not handle Space"
    );
}

#[test]
fn test_hover_reports_the_uri_only_for_a_live_link() {
    // Both models are built before the harness so the harness's view
    // lifetime is the test's own, and either link can be swapped in later.
    let live = HyperlinkModel::new("Hover me", "https://preview.url");
    let dead = HyperlinkModel::new("Disabled hover", "https://disabled.url").disabled();

    // Case 1 — entering reports the uri, leaving reports its absence.
    let mut harness = Harness::headless(|| {
        Hyperlink::controlled(Controlled::new(&live), "link_hover", |emission| {
            Some(TestMsg::Emitted(emission))
        })
        .into()
    });

    let bounds = widget_bounds(&mut harness);
    let over = centre(bounds);

    let enter_msgs = harness.hover(over);
    assert_eq!(
        enter_msgs,
        vec![TestMsg::Emitted(HyperlinkEmission::HoverUri(Some(
            "https://preview.url".to_owned()
        )))],
        "[link_hover] cursor entering emits HoverUri(Some(..))"
    );

    let exit_msgs = harness.hover(FAR_AWAY);
    assert_eq!(
        exit_msgs,
        vec![TestMsg::Emitted(HyperlinkEmission::HoverUri(None))],
        "[link_hover] cursor leaving emits HoverUri(None)"
    );

    // Case 2 — a disabled controlled link registers nothing and previews
    // nothing.
    harness.set_view(|| {
        Hyperlink::controlled(Controlled::new(&dead), "link_dis_hover", |emission| {
            Some(TestMsg::Emitted(emission))
        })
        .into()
    });

    assert!(
        harness.recorded().is_empty(),
        "[link_dis_hover] a disabled controlled link must not register with focus"
    );

    let enter_msgs = harness.hover(Point::new(10.0, 10.0));
    assert!(
        enter_msgs.is_empty(),
        "[link_dis_hover] a disabled controlled link must not emit a hover uri preview"
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

    // One renderer drives the whole 2 x 2 x 2 grid of states. Each cell
    // builds its link under its own focus id, so the harness's carried
    // widget state cannot leak from one cell into the next.
    let mut harness = Harness::headless(|| {
        Hyperlink::<TestMsg>::standalone(
            "Test Link",
            "https://example.com",
            "link_render_1_0_0",
            |_| None,
        )
        .enabled(true)
        .mount(p.surface_base)
        .into()
    });

    for (index, (enabled, focused, mount)) in [
        (true, false, p.surface_base),
        (true, false, p.surface_overlay),
        (true, true, p.surface_base),
        (true, true, p.surface_overlay),
        (false, false, p.surface_base),
        (false, false, p.surface_overlay),
        (false, true, p.surface_base),
        (false, true, p.surface_overlay),
    ]
    .into_iter()
    .enumerate()
    {
        let focus_id = format!("link_render_{index}_{}", if enabled { 1 } else { 0 });
        // The view builder is called once per frame, so it gets its own copy
        // of the id; the case keeps `focus_id` to aim the keyboard at.
        let mounted_focus = focus_id.clone();

        harness.set_view(move || {
            let link = Hyperlink::<TestMsg>::standalone(
                "Test Link",
                "https://example.com",
                mounted_focus.clone(),
                |_| None,
            )
            .enabled(enabled)
            .mount(mount);

            link.into()
        });

        if focused && enabled {
            focus_one(&mut harness, &focus_id);
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

        // Reaching the end of a cell is the claim: every state painted
        // through the real renderer without panicking. There is nothing to
        // compare the pixels against here — the state-specific claims live
        // in the resolver and layer tests above.
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
